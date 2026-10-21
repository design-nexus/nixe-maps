use crate::integration::{DeepLink, MapCommand, parse_deep_link, start_dbus_server};
use crate::theme;
use crate::ui::MainWindow;
use gio::prelude::*;
use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

pub struct MapsApp {
    app: gtk::Application,
    main_window: Rc<RefCell<Option<Rc<MainWindow>>>>,
    cmd_receiver: Rc<RefCell<Option<mpsc::Receiver<MapCommand>>>>,
}

impl MapsApp {
    pub fn new() -> Self {
        let app = gtk::Application::builder()
            .application_id("io.github.design_nexus.nixe.Maps")
            .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE | gio::ApplicationFlags::HANDLES_OPEN)
            .build();

        let (cmd_tx, cmd_rx) = mpsc::channel();

        // Spawn D-Bus server in background tokio runtime
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                let _ = start_dbus_server(cmd_tx).await;
            });
        });

        Self {
            app,
            main_window: Rc::new(RefCell::new(None)),
            cmd_receiver: Rc::new(RefCell::new(Some(cmd_rx))),
        }
    }

    pub fn run(&self) -> glib::ExitCode {
        self.app.connect_startup(|_| {
            theme::install();
        });

        let win_ref = self.main_window.clone();
        let cmd_rx_cell = self.cmd_receiver.clone();

        self.app.connect_activate(move |application| {
            if win_ref.borrow().is_none() {
                let win = MainWindow::build(application);
                win.window.present();
                *win_ref.borrow_mut() = Some(win.clone());

                // Poll for incoming D-Bus commands on GTK main loop
                if let Some(rx) = cmd_rx_cell.borrow_mut().take() {
                    let win_for_cmds = win.clone();
                    glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
                        while let Ok(cmd) = rx.try_recv() {
                            match cmd {
                                MapCommand::ShowLocation { lat, lon, label } => {
                                    win_for_cmds.open_location(lat, lon, &label);
                                    win_for_cmds.window.present();
                                }
                                MapCommand::Search { query } => {
                                    win_for_cmds.search_query(&query);
                                    win_for_cmds.window.present();
                                }
                                MapCommand::ShowRoute { start, end, mode } => {
                                    win_for_cmds.open_route(start, end, &mode);
                                    win_for_cmds.window.present();
                                }
                            }
                        }
                        glib::ControlFlow::Continue
                    });
                }
            } else if let Some(ref win) = *win_ref.borrow() {
                win.window.present();
            }
        });

        // Command line handling
        let win_cmd = self.main_window.clone();
        self.app.connect_command_line(move |application, cmdline| {
            let arguments = cmdline.arguments();
            let args: Vec<String> = arguments.iter().map(|s| s.to_string_lossy().to_string()).collect();

            application.activate();

            if args.len() > 1 {
                let arg = &args[1];
                if let Some(ref win) = *win_cmd.borrow() {
                    if let Some(link) = parse_deep_link(arg) {
                        match link {
                            DeepLink::Location { lat, lon, label } => {
                                win.open_location(lat, lon, &label);
                            }
                            DeepLink::Search { query } => {
                                win.search_query(&query);
                            }
                            DeepLink::Route { start, end, mode } => {
                                win.open_route(start, end, &mode);
                            }
                        }
                    } else if !arg.starts_with('-') {
                        win.search_query(arg);
                    }
                }
            }

            glib::ExitCode::SUCCESS
        });

        // Open URI handling (e.g. geo: URIs from browser or terminal)
        let win_open = self.main_window.clone();
        self.app.connect_open(move |application, files, _hint| {
            application.activate();
            for file in files {
                let uri = file.uri().to_string();
                if let Some(ref win) = *win_open.borrow() {
                    if let Some(link) = parse_deep_link(&uri) {
                        match link {
                            DeepLink::Location { lat, lon, label } => {
                                win.open_location(lat, lon, &label);
                            }
                            DeepLink::Search { query } => {
                                win.search_query(&query);
                            }
                            DeepLink::Route { start, end, mode } => {
                                win.open_route(start, end, &mode);
                            }
                        }
                    }
                }
            }
        });

        self.app.run()
    }
}
