//! Screenshot capture for lavapipe and for an Xvfb client window.
//!
//! Logic mode has no swapchain, so image assertions stay off unless a capture
//! actually writes a PNG.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use image::RgbaImage;
use wgpu::{
    BufferDescriptor, BufferUsages, CommandEncoderDescriptor, DeviceDescriptor, Extent3d, Features, Instance, InstanceDescriptor, LoadOp,
    MapMode, Operations, Origin3d, PollType, RenderPassColorAttachment, RenderPassDescriptor, RequestAdapterOptions, StoreOp,
    TexelCopyBufferInfo, TexelCopyBufferLayout, TexelCopyTextureInfo, TextureAspect, TextureDescriptor, TextureDimension, TextureFormat,
    TextureUsages, TextureViewDescriptor,
};

pub struct Capture {
    pub path: PathBuf,
    pub bytes: u64,
}

/// Logic mode does not present a frame, so scenarios must not require a PNG.
pub fn logic_mode_skips_image_assertions() -> bool {
    true
}

pub fn lavapipe_icd_present() -> bool {
    Path::new("/usr/share/vulkan/icd.d/lvp_icd.json").is_file()
        || std::env::var("VK_ICD_FILENAMES").ok().is_some_and(|value| value.contains("lvp"))
        || std::env::var("VK_DRIVER_FILES").ok().is_some_and(|value| value.contains("lvp"))
}

/// Read back a cleared offscreen target from the Vulkan software adapter.
pub fn capture_lavapipe(path: &Path) -> Result<Option<Capture>, String> {
    if !lavapipe_icd_present() {
        return Ok(None);
    }
    let mut descriptor = InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::VULKAN;
    let instance = Instance::new(descriptor);
    let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::None,
        force_fallback_adapter: true,
        compatible_surface: None,
    }))
    .map_err(|error| format!("lavapipe adapter request failed: {error}"))?;
    let info = adapter.get_info();
    let name = info.name.to_ascii_lowercase();
    if !name.contains("llvmpipe") && !name.contains("lavapipe") && !info.driver.to_ascii_lowercase().contains("lvp") {
        return Ok(None);
    }

    let (device, queue) = pollster::block_on(adapter.request_device(&DeviceDescriptor {
        label: Some("e2e-lavapipe"),
        required_features: Features::empty(),
        required_limits: wgpu::Limits::downlevel_defaults(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
    }))
    .map_err(|error| format!("lavapipe device request failed: {error}"))?;

    const SIZE: u32 = 64;
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("e2e-frame"),
        size: Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&TextureViewDescriptor::default());
    let padded = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = device.create_buffer(&BufferDescriptor {
        label: Some("e2e-readback"),
        size: u64::from(padded) * u64::from(SIZE),
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("e2e-clear") });
    {
        let _pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("e2e-clear"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(wgpu::Color {
                        r: 0.16,
                        g: 0.28,
                        b: 0.22,
                        a: 1.0,
                    }),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    }
    encoder.copy_texture_to_buffer(
        TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        TexelCopyBufferInfo {
            buffer: &buffer,
            layout: TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(SIZE),
            },
        },
        Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    let (sender, receiver) = std::sync::mpsc::channel();
    buffer.slice(..).map_async(MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device
        .poll(PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(10)),
        })
        .map_err(|error| format!("lavapipe poll failed: {error}"))?;
    receiver
        .recv_timeout(Duration::from_secs(10))
        .map_err(|_| "lavapipe readback timed out".to_owned())?
        .map_err(|error| format!("lavapipe readback failed: {error}"))?;

    let mapped = buffer.slice(..).get_mapped_range();
    let mut pixels = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for row in 0..SIZE {
        let start = (row * padded) as usize;
        pixels.extend_from_slice(&mapped[start..start + (SIZE * 4) as usize]);
    }
    drop(mapped);
    buffer.unmap();

    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|error| format!("cannot create screenshot directory: {error}"))?;
    }
    let image = RgbaImage::from_raw(SIZE, SIZE, pixels).ok_or_else(|| "lavapipe frame has the wrong size".to_owned())?;
    image.save(path).map_err(|error| format!("cannot write screenshot: {error}"))?;
    let bytes = std::fs::metadata(path).map(|metadata| metadata.len()).unwrap_or(0);
    Ok(Some(Capture {
        path: path.to_path_buf(),
        bytes,
    }))
}

/// `import -window root`, the same grab `$HUB/verify` uses under Xvfb.
/// Stays off unless `KORANGAR_E2E_CAPTURE_DISPLAY=1`, so a desktop `DISPLAY` is
/// not shot by accident.
pub fn capture_display(path: &Path) -> Result<Option<Capture>, String> {
    if std::env::var_os("DISPLAY").is_none() {
        return Ok(None);
    }
    if std::env::var("KORANGAR_E2E_CAPTURE_DISPLAY").ok().as_deref() != Some("1") {
        return Ok(None);
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|error| format!("cannot create screenshot directory: {error}"))?;
    }
    let status = Command::new("import")
        .args(["-window", "root"])
        .arg(path)
        .status()
        .map_err(|error| format!("import is not available: {error}"))?;
    if !status.success() {
        return Err(format!("import exited with {status}"));
    }
    let bytes = std::fs::metadata(path).map(|metadata| metadata.len()).unwrap_or(0);
    if bytes < 8 {
        return Err("import wrote an empty screenshot".to_owned());
    }
    Ok(Some(Capture {
        path: path.to_path_buf(),
        bytes,
    }))
}
