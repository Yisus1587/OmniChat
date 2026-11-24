//! OmniChat — launcher de mensajería: cada servicio es su web oficial en un webview aislado.

mod advanced;
mod backup;
mod catalog;
mod commands;
mod db;
mod error;
mod host;
mod lock;
mod memory;
mod prefs;
mod settings;
mod shortcuts;
mod siteicon;
mod sync;
mod toast;
mod tray;
mod window_state;

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::{Manager, WindowEvent};
use tracing_subscriber::EnvFilter;

use crate::host::{ServiceHost, MAIN_WINDOW};

/// Argumento con el que Windows arranca OmniChat al iniciar sesión.
const MINIMIZED_ARG: &str = "--minimized";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("omnichat_lib=info,warn")),
        )
        .init();

    tauri::Builder::default()
        // Debe ir primero: si OmniChat ya está abierto, la segunda copia sólo lo muestra.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| tray::show_main(app)))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![MINIMIZED_ARG]),
        ))
        .plugin(shortcuts::plugin())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let data = app.path().app_data_dir()?;
            let pool = tauri::async_runtime::block_on(db::init(&data.join("launcher.db")))?;
            let settings = tauri::async_runtime::block_on(prefs::load(&pool));
            app.manage(pool);
            app.manage(ServiceHost::new(data.join("webviews"), settings.clone()));
            tray::build(app.handle())?;
            toast::register_app(app.handle());
            // Con PIN, OmniChat arranca bloqueado.
            tauri::async_runtime::block_on(lock::init(app.handle()));
            prefs::apply(app.handle(), &settings);

            // Webs personalizadas sin icono (añadidas antes o sin conexión): buscarlo ahora.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let pool = handle.state::<sqlx::SqlitePool>();
                for s in db::list(&pool).await.unwrap_or_default() {
                    if s.icon.is_none() {
                        siteicon::refresh(&handle, &s);
                    }
                }
            });

            if let Some(window) = app.get_window(MAIN_WINDOW) {
                window_state::restore(app.handle(), &window);
                // Arranque con Windows: directo a la bandeja. Si no, se muestra.
                if !std::env::args().any(|a| a == MINIMIZED_ARG) {
                    let _ = window.show();
                    let _ = window.set_focus();
                }

                let handle = app.handle().clone();
                let hinted = AtomicBool::new(false);
                window.on_window_event(move |event| match event {
                    // Cerrar = esconder en la bandeja (los servicios siguen recibiendo mensajes).
                    WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        if let Some(w) = handle.get_window(MAIN_WINDOW) {
                            window_state::save(&handle, &w);
                            let _ = w.hide();
                        }
                        lock::on_hidden(&handle);
                        if !hinted.swap(true, Ordering::Relaxed) {
                            toast::show(
                                &handle,
                                toast::Notice {
                                    service_id: None,
                                    title: "OmniChat sigue abierto".into(),
                                    body: "Lo tienes junto al reloj. Para cerrarlo del todo: clic derecho en el icono → Salir."
                                        .into(),
                                    sound: None,
                                },
                            );
                        }
                    }
                    // Atajos de teclado sólo mientras OmniChat está en primer plano.
                    WindowEvent::Focused(focused) => {
                        shortcuts::set_active(&handle, *focused);
                        lock::on_focus(&handle, *focused);
                    }
                    _ => {}
                });
            }

            // Tareas periódicas: estado de "Concentración", hibernación y bloqueo por inactividad.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(30)).await;
                    let host = handle.state::<ServiceHost>();
                    tray::refresh_focus(&handle, &host.settings());
                    host.hibernate_idle(&handle);
                    lock::check_idle(&handle);
                    sync::tick(&handle).await;
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_services,
            commands::sync_layout,
            commands::add_service,
            commands::update_service,
            commands::reorder_services,
            commands::remove_service,
            commands::reload_service,
            commands::go_home_service,
            commands::open_in_browser,
            commands::unread_counts,
            commands::list_workspaces,
            commands::create_workspace,
            commands::rename_workspace,
            commands::delete_workspace,
            commands::get_settings,
            commands::update_settings,
            commands::set_focus,
            commands::set_service_icon,
            commands::list_notifications,
            commands::clear_notifications,
            commands::hibernate_service,
            commands::list_hibernated,
            commands::lock_status,
            commands::lock_now,
            commands::unlock,
            commands::set_pin,
            commands::export_config,
            commands::import_config,
            commands::refresh_service_icon,
            commands::service_memory,
            commands::extensions_info,
            commands::open_extensions_folder,
            commands::reload_extensions,
            commands::sync_suggestions,
            commands::set_sync_folder,
            commands::sync_now,
            commands::list_tasks,
            commands::add_task,
            commands::set_task_done,
            commands::delete_task,
            commands::clear_done_tasks,
        ])
        .run(tauri::generate_context!())
        .expect("error while running OmniChat");
}
