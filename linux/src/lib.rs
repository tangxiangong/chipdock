pub mod backend;
pub mod model;
pub use chipdock_gpui_update as update;
pub mod instance;
pub mod platform;
pub mod presentation;
pub mod serial;

mod ui;
mod workbench;
use nsh_core::prelude::*;

/// Native Rust workbench components.
#[native_module("chipdock/native")]
fn native(registry: &mut Registry) {
    registry.register::<App>();
}

/// ChipDock's desktop workbench.
#[component]
pub fn App() -> impl IntoElement {
    #[cfg(debug_assertions)]
    if let Some(preview) = verified_update_preview() {
        return match preview {
            Ok(feed) => ui::preview_available(feed).into_element(),
            Err(error) => view()
                .p_6()
                .child(text(error).text_color(move || nsh_base::theme().destructive))
                .into_element(),
        };
    }
    match initialize_desktop() {
        Ok(true) => ui::workbench().into_element(),
        Ok(false) => {
            effect(nsh_core::element::desktop::quit);
            view().into_element()
        }
        Err(error) => view()
            .p_6()
            .child(text(error).text_color(move || nsh_base::theme().destructive))
            .into_element(),
    }
}

/// Initializes process ownership and update/driver handoffs for both the custom binary and NSH CLI runners.
pub fn initialize_desktop() -> Result<bool, String> {
    static START: std::sync::OnceLock<Result<bool, String>> = std::sync::OnceLock::new();
    static INSTANCE: std::sync::OnceLock<instance::Instance> = std::sync::OnceLock::new();
    START
        .get_or_init(|| {
            if let Some(result) = platform::handle_driver_install() {
                result.map_err(|e| format!("{e:#}"))?;
                return Ok(false);
            }
            if update::finish_update(&std::env::args().skip(1).collect::<Vec<_>>())
                .map_err(|e| format!("更新失败：{e:#}"))?
            {
                return Ok(false);
            }
            let firmware = instance::firmware_argument();
            match instance::Instance::acquire(
                std::env::args().any(|a| a == "--after-update"),
                firmware.as_deref(),
            )
            .map_err(|e| format!("无法启动 ChipDock：{e:#}"))?
            {
                Some(instance) => {
                    let _ = INSTANCE.set(instance);
                    Ok(true)
                }
                None => Ok(false),
            }
        })
        .clone()
}

#[cfg(debug_assertions)]
fn verified_update_preview() -> Option<Result<update::Feed, String>> {
    let feed_path = std::env::var_os("CHIPDOCK_UPDATE_PREVIEW_FEED_FILE")?;
    Some(
        (|| -> anyhow::Result<update::Feed> {
            let prefs = std::env::var_os("CHIPDOCK_PREFERENCES_PATH")
                .ok_or_else(|| anyhow::anyhow!("只读更新预览需要独立配置路径"))?;
            let prefs = std::path::PathBuf::from(prefs);
            let feed_path = std::path::PathBuf::from(feed_path);
            anyhow::ensure!(
                prefs.is_absolute() && feed_path.is_absolute(),
                "只读更新预览路径必须是绝对路径"
            );
            use std::io::Read;
            let mut bytes = Vec::new();
            std::fs::File::open(feed_path)?
                .take(1024 * 1024 + 1)
                .read_to_end(&mut bytes)?;
            anyhow::ensure!(bytes.len() <= 1024 * 1024, "更新清单超过 1 MiB");
            update::verify(&bytes, update::PUBLIC_KEY)
        })()
        .map_err(|error| format!("只读更新预览失败：{error:#}")),
    )
}
