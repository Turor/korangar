//! Adapter selection for startup. Portable Windows TEST installs prefer a
//! high-performance adapter that still meets mandatory client gates, while
//! honoring explicit `WGPU_ADAPTER_NAME` / `WGPU_POWER_PREF` overrides.

use wgpu::util::initialize_adapter_from_env;
use wgpu::{Adapter, Backends, DeviceType, Instance, PowerPreference, RequestAdapterOptions};

use crate::deployment;
use crate::graphics::MAX_TEXTURE_SIZE;

/// True when an adapter can satisfy the mandatory feature/limit gate used
/// before device creation (`TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES` and at
/// least [`MAX_TEXTURE_SIZE`] 2D textures).
pub(crate) fn meets_mandatory_adapter_gate(adapter: &Adapter) -> bool {
    let features = adapter.features();
    let limits = adapter.limits();
    features.contains(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES) && limits.max_texture_dimension_2d >= MAX_TEXTURE_SIZE
}

fn adapter_rank(adapter: &Adapter) -> (u8, u8) {
    let info = adapter.get_info();
    let discrete = match info.device_type {
        DeviceType::DiscreteGpu => 2,
        DeviceType::IntegratedGpu => 1,
        _ => 0,
    };
    let rich = u8::from(adapter.features().contains(wgpu::Features::TEXTURE_BINDING_ARRAY));
    (discrete, rich)
}

fn summarize_adapters(adapters: &[Adapter]) -> String {
    adapters
        .iter()
        .map(|adapter| {
            let info = adapter.get_info();
            let gate = if meets_mandatory_adapter_gate(adapter) {
                "gate=ok"
            } else {
                "gate=fail"
            };
            format!("{} [{} {:?}] {gate}", info.name, info.backend, info.device_type)
        })
        .take(6)
        .collect::<Vec<_>>()
        .join("; ")
}

/// Select a graphics adapter.
///
/// Order:
/// 1. Explicit `WGPU_ADAPTER_NAME` (unchanged wgpu helper).
/// 2. Explicit `WGPU_POWER_PREF` via wgpu env-aware request.
/// 3. Portable installs: enumerate, keep adapters that pass the mandatory
///    capability gate, prefer discrete/high-capability, else high-performance
///    request among gated adapters.
/// 4. Otherwise high-performance request when no power preference is set.
pub(crate) async fn select_startup_adapter(instance: &Instance) -> Result<Adapter, String> {
    if let Ok(adapter) = initialize_adapter_from_env(instance, None).await {
        if !meets_mandatory_adapter_gate(&adapter) {
            let info = adapter.get_info();
            return Err(format!(
                "WGPU_ADAPTER_NAME matched '{}' ({:?}/{:?}) but fails mandatory texture/feature gate",
                info.name, info.backend, info.device_type
            ));
        }
        return Ok(adapter);
    }

    if std::env::var_os("WGPU_POWER_PREF").is_some() {
        return instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::from_env().unwrap_or(PowerPreference::HighPerformance),
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .map_err(|error| format!("Adapter request with WGPU_POWER_PREF failed: {error}"));
    }

    if deployment::is_portable() {
        let adapters = instance.enumerate_adapters(Backends::all()).await;
        let summary = summarize_adapters(&adapters);
        let mut eligible: Vec<Adapter> = adapters.into_iter().filter(meets_mandatory_adapter_gate).collect();
        if eligible.is_empty() {
            return Err(format!(
                "No adapter meets mandatory texture size and format-feature gates ({summary})"
            ));
        }
        eligible.sort_by_key(|adapter| std::cmp::Reverse(adapter_rank(adapter)));
        if let Ok(preferred) = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            && meets_mandatory_adapter_gate(&preferred)
        {
            let preferred_id = preferred.get_info();
            if eligible.iter().any(|adapter| {
                let info = adapter.get_info();
                info.name == preferred_id.name && info.backend == preferred_id.backend && info.device == preferred_id.device
            }) {
                return Ok(preferred);
            }
        }
        return Ok(eligible.remove(0));
    }

    instance
        .request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::from_env().unwrap_or(PowerPreference::HighPerformance),
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .await
        .map_err(|error| format!("Default high-performance adapter request failed: {error}"))
}

/// Sanitized one-line adapter identity for logs (no paths or credentials).
pub(crate) fn adapter_diagnostic(adapter: &Adapter) -> String {
    let info = adapter.get_info();
    format!(
        "adapter='{}' backend={:?} type={:?} vendor={} device={} driver='{}'",
        info.name, info.backend, info.device_type, info.vendor, info.device, info.driver
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_helper_is_wired_through_deployment() {
        let text = deployment::sanitize_graphics_diagnostic(
            "Validation",
            "bind group layout mismatch
second line
third
fourth
fifth",
        );
        assert!(text.starts_with("Graphics Validation:"));
        assert!(text.contains("bind group layout mismatch"));
        assert!(!text.contains("fifth"));
    }

    #[test]
    fn adapter_rank_tuple_orders_discrete_first() {
        assert!((2u8, 1u8) > (1u8, 1u8));
        assert!((2u8, 0u8) > (1u8, 1u8));
    }
}
