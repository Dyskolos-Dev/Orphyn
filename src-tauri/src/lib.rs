//! Orphyn's native core (Tauri v2). The React renderer talks to it through the bridge in
//! `src/renderer/src/bridge/`, which installs `window.orphyn` with the same interface as the
//! Electron preload. Every domain lives in its own module and exposes `init(app)` plus its
//! commands; this file only composes them (see `docs/TAURI.md`).

pub mod accounts;
pub mod cast;
pub mod discord;
pub mod error;
pub mod media_controls;
pub mod pip;
pub mod updater;
pub mod window;

use tauri::{Manager, RunEvent};
use tauri_plugin_window_state::StateFlags;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        // Must be the first plugin: a second launch only focuses the running window.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            window::focus_main(app);
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(
            tauri_plugin_window_state::Builder::default()
                // Visibility is handled below, once the bounds are restored.
                .with_state_flags(StateFlags::all() & !StateFlags::VISIBLE)
                .build(),
        )
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_os::init())
        .setup(|app| {
            let handle = app.handle();
            // Accounts first: the renderer asks for them as soon as it loads.
            accounts::init(handle)?;
            updater::init(handle)?;
            cast::init(handle)?;
            discord::init(handle)?;
            media_controls::init(handle)?;
            pip::init(handle)?;
            window::init(handle)?;
            if let Some(main) = app.get_webview_window(window::MAIN) {
                main.show()?;
                main.set_focus()?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // window
            window::window_minimize,
            window::window_toggle_maximize,
            window::window_close,
            window::window_get_state,
            window::window_set_fullscreen,
            window::window_set_appearance,
            // accounts
            accounts::accounts_list,
            accounts::accounts_add,
            accounts::accounts_rename,
            accounts::accounts_remove,
            accounts::accounts_set_active,
            accounts::accounts_get_active,
            accounts::accounts_get_token,
            accounts::accounts_get_device_id,
            accounts::accounts_get_status,
            // updater
            updater::updater_get_state,
            updater::updater_check,
            updater::updater_install,
            updater::updater_open_release,
            updater::updater_set_auto_update,
            // cast (stubs)
            cast::cast_start_discovery,
            cast::cast_stop_discovery,
            cast::cast_list_devices,
            cast::cast_get_session,
            cast::cast_load,
            cast::cast_play,
            cast::cast_pause,
            cast::cast_seek,
            cast::cast_stop,
            cast::cast_set_volume,
            cast::cast_set_muted,
            cast::cast_disconnect,
            // discord (stubs)
            discord::discord_get_state,
            discord::discord_set_presence,
            discord::discord_set_settings,
            // media controls (stubs)
            media_controls::media_controls_set_metadata,
            media_controls::media_controls_set_playback,
            media_controls::media_controls_clear,
            // picture-in-picture (stubs)
            pip::pip_get_state,
            pip::pip_open,
            pip::pip_close,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Orphyn");

    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            updater::install_on_exit(handle);
        }
    });
}
