mod commands;
mod models;
mod services;
mod state;

use std::sync::atomic::Ordering;

use commands::{integrations, processes, projects, ssh};
use services::ProjectRepository;
use state::AppState;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, RunEvent, WindowEvent,
};

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let config_path = app.path().app_config_dir()?.join("projects.json");
            let repository = ProjectRepository::load(config_path)
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
            app.manage(AppState::new(repository));

            let show_item =
                MenuItem::with_id(app, "show", "Mostrar Server Manager", true, None::<&str>)?;
            let stop_all_item =
                MenuItem::with_id(app, "stop_all", "Detener todos", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Detener y salir", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &stop_all_item, &quit_item])?;

            TrayIconBuilder::with_id("server-manager")
                .icon(tray_icon())
                .icon_as_template(true)
                .tooltip("Server Manager")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_main_window(app),
                    "stop_all" => {
                        let handle = app.clone();
                        tauri::async_runtime::spawn(async move {
                            processes::stop_all_for_app(handle).await;
                        });
                    }
                    "quit" => begin_quit(app.clone()),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main_window(tray.app_handle());
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<AppState>();
                if !state.quitting.load(Ordering::SeqCst) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            projects::list_projects,
            projects::save_project,
            projects::delete_project,
            integrations::open_project_in_code,
            integrations::open_service_url,
            integrations::open_project_terminal,
            ssh::open_ssh_terminal,
            processes::get_process_states,
            processes::start_service,
            processes::stop_service,
            processes::restart_service,
            processes::start_project,
            processes::stop_project,
            processes::restart_project,
            processes::send_input,
            processes::start_all,
            processes::stop_all,
        ])
        .build(tauri::generate_context!())
        .expect("error al construir Server Manager");

    app.run(|app_handle, event| {
        if let RunEvent::ExitRequested { api, .. } = event {
            let state = app_handle.state::<AppState>();
            if !state.quitting.load(Ordering::SeqCst) {
                api.prevent_exit();
                begin_quit(app_handle.clone());
            }
        }
    });
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn begin_quit(app: tauri::AppHandle) {
    let should_begin = {
        let state = app.state::<AppState>();
        !state.quitting.swap(true, Ordering::SeqCst)
    };
    if !should_begin {
        return;
    }

    tauri::async_runtime::spawn(async move {
        processes::stop_all_for_app(app.clone()).await;
        app.exit(0);
    });
}

fn tray_icon() -> Image<'static> {
    const WIDTH: u32 = 18;
    const HEIGHT: u32 = 18;
    let mut rgba = vec![0_u8; (WIDTH * HEIGHT * 4) as usize];

    for (top, bottom) in [(2_u32, 6_u32), (7, 11), (12, 16)] {
        for y in top..bottom {
            for x in 2..16 {
                let index = ((y * WIDTH + x) * 4) as usize;
                rgba[index] = 255;
                rgba[index + 1] = 255;
                rgba[index + 2] = 255;
                rgba[index + 3] = if x < 5 { 210 } else { 255 };
            }
        }
    }

    Image::new_owned(rgba, WIDTH, HEIGHT)
}
