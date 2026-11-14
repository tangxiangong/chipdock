#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
use nsh_platform_linux as platform;
#[cfg(target_os = "macos")]
use nsh_platform_macos as platform;
#[cfg(target_os = "windows")]
use nsh_platform_windows as platform;

fn main() -> std::process::ExitCode {
    match chipdock_nsh::initialize_desktop() {
        Ok(true) => {}
        Ok(false) => return std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            return std::process::ExitCode::FAILURE;
        }
    }
    platform::set_ui_sets(vec![nsh_base::registry(), chipdock_nsh::registry()]);
    let isolated_data = std::env::var_os("CHIPDOCK_PREFERENCES_PATH").and_then(|path| {
        std::path::PathBuf::from(path)
            .parent()
            .map(|parent| parent.join("nsh-state"))
    });
    let isolated_cache = isolated_data
        .as_ref()
        .map(|path| platform::CacheOptions::at(platform::CacheLocation::Dir(path.join("cache"))))
        .unwrap_or_default();
    let result = platform::run_app(
        platform::AppSource::Rust(platform::Root::of::<chipdock_nsh::App>().main_window_json(r#"{"titleBar":{"style":"hidden","height":52,"controlsOffset":{"x":19,"y":19}},"minSize":{"width":900,"height":652},"closeOnPress":false,"restoreBounds":true}"#)),
        platform::AppOptions {
            title: "ChipDock".into(),
            app_id: if cfg!(target_os = "linux") {
                "com.xiaoyu.chipdock"
            } else {
                "com.xiaoyu.chipdock.gpui"
            }
            .into(),
            size: (1160, 780),
            data_dir:isolated_data,
            cache:isolated_cache,
            ..Default::default()
        },
    );
    if let Err(error) = chipdock_service::serial::serial_shutdown() {
        eprintln!("{error}");
    }
    #[cfg(target_os = "linux")]
    let result = result.into();
    result
}
