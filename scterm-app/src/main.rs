//! Binary entry point for `scterm`.

fn main() {
    std::process::exit(scterm_app::run_cli(std::env::args()));
}
