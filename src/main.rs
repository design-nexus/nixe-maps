mod app;
mod integration;
mod location;
mod providers;
mod settings;
mod settings_dialog;
mod storage;
mod theme;
mod ui;

use app::MapsApp;
use integration::handle_cli_api;

fn main() -> glib::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Headless API query check for Nixe Assistant / scripts
    if !args.is_empty() && args[0] == "--api" {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to initialize async runtime");

        let handled = rt.block_on(async { handle_cli_api(&args).await });
        if handled {
            return glib::ExitCode::SUCCESS;
        }
    }

    let maps_app = MapsApp::new();
    maps_app.run()
}
