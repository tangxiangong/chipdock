use gpui_kit::{AssetSource, SharedString};
use std::borrow::Cow;

const ICONS: &[(&str, &[u8])] = &[
    ("table", include_bytes!("../assets/icons/table.svg")),
    ("palette", include_bytes!("../assets/icons/palette.svg")),
    ("filter", include_bytes!("../assets/icons/filter.svg")),
    ("history", include_bytes!("../assets/icons/history.svg")),
    ("firmware", include_bytes!("../assets/icons/firmware.svg")),
    ("file", include_bytes!("../assets/icons/file.svg")),
    ("folder", include_bytes!("../assets/icons/folder.svg")),
    ("usb", include_bytes!("../assets/icons/usb.svg")),
    ("link", include_bytes!("../assets/icons/link.svg")),
    ("unlink", include_bytes!("../assets/icons/unlink.svg")),
    ("refresh", include_bytes!("../assets/icons/refresh.svg")),
    ("search", include_bytes!("../assets/icons/search.svg")),
    ("sidebar", include_bytes!("../assets/icons/sidebar.svg")),
    ("composer", include_bytes!("../assets/icons/composer.svg")),
    ("terminal", include_bytes!("../assets/icons/terminal.svg")),
    ("write", include_bytes!("../assets/icons/write.svg")),
    ("shield", include_bytes!("../assets/icons/shield.svg")),
    ("send", include_bytes!("../assets/icons/send.svg")),
    ("repeat", include_bytes!("../assets/icons/repeat.svg")),
    ("export", include_bytes!("../assets/icons/export.svg")),
    ("trash", include_bytes!("../assets/icons/trash.svg")),
    ("erase", include_bytes!("../assets/icons/erase.svg")),
    ("settings", include_bytes!("../assets/icons/settings.svg")),
    ("logs", include_bytes!("../assets/icons/logs.svg")),
    ("memory", include_bytes!("../assets/icons/memory.svg")),
    ("bookmark", include_bytes!("../assets/icons/bookmark.svg")),
    ("plot", include_bytes!("../assets/icons/plot.svg")),
    ("copy", include_bytes!("../assets/icons/copy.svg")),
    ("pause", include_bytes!("../assets/icons/pause.svg")),
    ("play", include_bytes!("../assets/icons/play.svg")),
    ("close", include_bytes!("../assets/icons/close.svg")),
    ("check", include_bytes!("../assets/icons/check.svg")),
    ("down", include_bytes!("../assets/icons/down.svg")),
    ("up", include_bytes!("../assets/icons/up.svg")),
    ("left", include_bytes!("../assets/icons/left.svg")),
    ("right", include_bytes!("../assets/icons/right.svg")),
    ("select", include_bytes!("../assets/icons/select.svg")),
    (
        "arrow-down",
        include_bytes!("../assets/icons/arrow-down.svg"),
    ),
    ("arrow-up", include_bytes!("../assets/icons/arrow-up.svg")),
    ("plus", include_bytes!("../assets/icons/plus.svg")),
    ("minus", include_bytes!("../assets/icons/minus.svg")),
    ("more", include_bytes!("../assets/icons/more.svg")),
    ("warning", include_bytes!("../assets/icons/warning.svg")),
    ("info", include_bytes!("../assets/icons/info.svg")),
    ("loader", include_bytes!("../assets/icons/loader.svg")),
    ("maximize", include_bytes!("../assets/icons/maximize.svg")),
    ("restore", include_bytes!("../assets/icons/restore.svg")),
    ("eye", include_bytes!("../assets/icons/eye.svg")),
    ("eye-off", include_bytes!("../assets/icons/eye-off.svg")),
    ("gear", include_bytes!("../assets/icons/gear.svg")),
    ("code", include_bytes!("../assets/icons/code.svg")),
    ("add-rule", include_bytes!("../assets/icons/add-rule.svg")),
    ("import", include_bytes!("../assets/icons/import.svg")),
    (
        "more-circle",
        include_bytes!("../assets/icons/more-circle.svg"),
    ),
    (
        "download-circle",
        include_bytes!("../assets/icons/download-circle.svg"),
    ),
];
pub struct Assets;

fn symbol(name: &str) -> &str {
    match name {
        "file-text" => "file",
        "folder-open" | "folder-closed" => "folder",
        "square-terminal" => "terminal",
        "panel-left" | "panel-left-open" | "panel-left-close" | "inspector" => "sidebar",
        "panel-bottom" | "panel-bottom-open" => "composer",
        "cpu" | "memory-stick" => "memory",
        "settings-2" => "settings",
        "refresh-cw" | "rotate-cw" => "refresh",
        "chevron-down" => "down",
        "chevron-up" => "up",
        "chevron-left" => "left",
        "chevron-right" => "right",
        "chevrons-up-down" => "select",
        "x" | "window-close" => "close",
        "window-minimize" | "minimize" | "dash" => "minus",
        "window-maximize" => "maximize",
        "window-restore" => "restore",
        "ellipsis" | "ellipsis-vertical" | "menu" => "more",
        "loader-circle" => "loader",
        "triangle-alert" | "circle-alert" => "warning",
        "circle-check" => "check",
        "circle-x" => "close",
        "arrow-left" | "undo" | "undo-2" => "left",
        "arrow-right" | "redo" | "redo-2" => "right",
        "external-link" => "export",
        "chart-pie" => "plot",
        "delete" => "trash",
        _ => name,
    }
}
impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if path == "app-icon.png" {
            return Ok(Some(Cow::Borrowed(include_bytes!(
                "../../windows/ChipDock/Assets/ChipDock.png"
            ))));
        }
        if path.is_empty() {
            return Ok(None);
        }
        let name = path
            .strip_prefix("icons/")
            .and_then(|s| s.strip_suffix(".svg"))
            .ok_or_else(|| anyhow::anyhow!("Unknown asset: {path}"))?;
        let name = symbol(name);
        let bytes = ICONS
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, bytes)| *bytes)
            .ok_or_else(|| anyhow::anyhow!("Missing SVG icon: {path}"))?;
        Ok(Some(Cow::Borrowed(bytes)))
    }
    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(ICONS
            .iter()
            .map(|(name, _)| format!("icons/{name}.svg"))
            .filter(|name| name.starts_with(path))
            .map(Into::into)
            .collect())
    }
}
