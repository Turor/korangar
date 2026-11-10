use clap::Parser;
use korangar::Client;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Arguments {
    /// Synchronize the asset cache and exit.
    #[arg(long)]
    sync_cache: bool,
    /// Check asset source and native clientinfo setup without a window/login.
    #[arg(long)]
    check_bootstrap: bool,
    /// Print a fixed token and exit. The signed updater runs this after
    /// swapping the executable and rolls back if it fails.
    #[arg(long)]
    update_self_check: bool,
}

fn main() {
    let arguments = Arguments::parse();
    if arguments.update_self_check {
        println!("update-self-check-ok");
        return;
    }
    if let Err(error) = startup_update() {
        eprintln!("Signed auto-update rejected: {error}");
        std::process::exit(2);
    }
    let interactive = !arguments.check_bootstrap && !arguments.sync_cache;
    if let Err(error) = korangar::deployment::prepare(interactive) {
        korangar::deployment::report_failure(&error, interactive);
        std::process::exit(2);
    }

    if arguments.check_bootstrap {
        match Client::check_bootstrap() {
            Ok(()) => {
                println!("Native bootstrap OK (configuration/source initialization only; no completeness, rendering or gameplay claim)")
            }
            Err(error) => {
                eprintln!("Native bootstrap failed: {error}");
                korangar::deployment::report_failure(
                    "Native bootstrap failed; restore approved assets or inspect console diagnostics",
                    false,
                );
                std::process::exit(2);
            }
        }
        return;
    }
    if let Some(mut client) = Client::init(arguments.sync_cache) {
        if let Err(error) = client.run() {
            eprintln!("Native client failed: {error}");
            korangar::deployment::report_failure(
                "Native client runtime failed; no credentials or packet contents recorded",
                interactive,
            );
            std::process::exit(2);
        }
    } else if !arguments.sync_cache {
        korangar::deployment::report_failure(
            "Client initialization failed; run korangar.exe --check-bootstrap for diagnostics",
            interactive,
        );
        std::process::exit(2);
    }
}

/// Checks the main-channel manifest unless `KORANGAR_SKIP_UPDATE=1`. The
/// default URL is `https://korangar-releases.turoran.com/main/manifest.jsonup` and may
/// be overridden with `KORANGAR_UPDATE_MANIFEST_URL`. An unreachable channel is
/// skipped so gameplay can start. A bad signature or an older build is fatal.
fn startup_update() -> Result<(), String> {
    let current_exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let install_root = current_exe.parent().ok_or("executable has no directory")?.to_path_buf();
    let relaunch_args: Vec<String> = std::env::args().skip(1).collect();
    let skip = std::env::var("KORANGAR_SKIP_UPDATE").ok();
    let manifest = std::env::var("KORANGAR_UPDATE_MANIFEST_URL")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let default = korangar_update::DEFAULT_UPDATE_MANIFEST_URL;
    let manifest_url = manifest.as_deref().unwrap_or(default);
    let outcome = korangar_update::run_startup_update(&korangar_update::StartupRequest {
        skip_env: skip.as_deref(),
        manifest_url: Some(manifest_url),
        current_version: env!("CARGO_PKG_VERSION"),
        install_root: &install_root,
        current_exe: &current_exe,
        public_key: &korangar_update::manifest::EMBEDDED_UPDATE_PUBLIC_KEY,
        relaunch_args: &relaunch_args,
        spawn_relaunch: true,
    })?;
    if matches!(outcome, korangar_update::StartupOutcome::Applied { relaunched: true }) {
        std::process::exit(0);
    }
    Ok(())
}
