//! Signs release manifests. Private keys come from a path or env var and must
//! live outside this repository. See `docs/signed-auto-update.md`.

fn main() {
    if let Err(error) = korangar_update::cli::dispatch(std::env::args().skip(1)) {
        eprintln!("korangar-sign-release: {error}");
        std::process::exit(1);
    }
}
