use hostly_lib::{cli, elevation};

fn main() {
    elevation::relaunch_as_admin_if_needed();

    if !cli::run_cli(None) {
        println!("Hostly CLI (Elevated): Windows CLI with auto-elevation support.");
        println!("Use --help to see available commands.");
    }
}
