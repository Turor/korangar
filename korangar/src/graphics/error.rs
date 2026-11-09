use korangar_debug::logging::{Colorize, print_debug};
use wgpu::Error;

use crate::deployment::{self, sanitize_graphics_diagnostic};

pub fn error_handler(error: Error) {
    let (message_type, message) = match &error {
        Error::OutOfMemory { source } => ("OutOfMemory", source.to_string()),
        Error::Validation { source, description } => ("Validation", format!("{source}: {description}")),
        Error::Internal { source, description } => ("Internal", format!("{source}: {description}")),
    };

    print_debug!("wgpu [{}] [{}]: {}", message_type.yellow(), "error".red(), message);

    let summary = sanitize_graphics_diagnostic(message_type, &message);
    deployment::report_failure(&summary, false);

    #[cfg(debug_assertions)]
    panic!("WGPU error found");
}
