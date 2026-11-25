pub mod backend;
pub mod instance;
pub mod model;
pub mod platform;
pub mod presentation;
pub mod serial;
pub mod update;

mod ui;
mod workbench;
use nsh_core::prelude::*;
use ui::WorkbenchApi;

/// Native Rust workbench components.
#[native_module("chipdock/native")]
fn native(registry: &mut Registry) {
    registry.register::<WorkbenchHost>();
    ui::register_regions(registry);
}

#[derive(Debug, Clone, PartialEq, Props)]
pub struct WorkbenchProps {
    /// Workbench content arranged by App.tsx.
    pub children: Prop<Children>,
    /// The settings window content, supplied by the TSX app.
    pub render_settings: Prop<Option<Renderer<String>>>,
    /// Composes the logs, memory and session plot windows.
    pub render_auxiliary: Prop<Option<Renderer<String>>>,
    /// Renders the current stage of the native update window.
    pub render_update: Prop<Option<Renderer<ui::UpdatePresentation>>>,
}

#[derive(Debug, Clone, PartialEq, Event)]
pub enum WorkbenchEvent {
    /// The visible mode, panels and document state changed.
    #[args(mode, sidebar, composer, modal, targetSetup, empty)]
    StateChanged(String, bool, bool, bool, bool, bool),
    /// Active tool and native sheet appearance.
    #[args(tool, nativeSheet)]
    ToolChange(Option<String>, bool),
}

/// Owns the desktop workbench session and delivers its layout state.
#[component(methods = WorkbenchApi)]
pub fn WorkbenchHost(props: WorkbenchProps, cx: Scope<WorkbenchEvent>) -> impl IntoElement {
    #[cfg(debug_assertions)]
    if let Some(preview) = verified_update_preview() {
        return match preview {
            Ok(feed) => ui::preview_available(feed, props.render_update).into_element(),
            Err(error) => view()
                .p_6()
                .child(text(error).text_color(move || nsh_base::theme().destructive))
                .into_element(),
        };
    }
    match initialize_desktop() {
        Ok(true) => ui::workbench(props, cx).into_element(),
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

/// Initializes instance ownership and driver handoffs for the packaged and development hosts.
pub fn initialize_desktop() -> Result<bool, String> {
    static START: std::sync::OnceLock<Result<bool, String>> = std::sync::OnceLock::new();
    static INSTANCE: std::sync::OnceLock<instance::Instance> = std::sync::OnceLock::new();
    START
        .get_or_init(|| {
            if let Some(result) = platform::handle_driver_install() {
                result.map_err(|e| format!("{e:#}"))?;
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
            std::fs::File::open(&feed_path)?
                .take(1024 * 1024 + 1)
                .read_to_end(&mut bytes)?;
            anyhow::ensure!(bytes.len() <= 1024 * 1024, "更新清单超过 1 MiB");
            let config: serde_json::Value =
                serde_json::from_str(include_str!("../shell-updater.json"))?;
            let public = config["publicKey"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("缺少更新公钥"))?;
            nsh_updater::verify(
                public,
                &bytes,
                &std::fs::read(feed_path.with_added_extension("sig"))?,
            )?;
            Ok(serde_json::from_slice(&bytes)?)
        })()
        .map_err(|error| format!("只读更新预览失败：{error:#}")),
    )
}
