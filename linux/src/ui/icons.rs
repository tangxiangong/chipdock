use nsh_core::{element::ImageSrc, prelude::*};

pub(super) fn source(name: &str) -> ImageSrc {
    match name {
        "select" => ImageSrc::svg_markup(include_str!("../../assets/icons/select.svg")),
        "target-select" => ImageSrc::svg_markup(
            &include_str!("../../assets/icons/select.svg")
                .replace("stroke-width=\"1.6\"", "stroke-width=\"1.8\"")
                .replace(
                    "<path",
                    "<path transform=\"translate(-8.4 -3.6) scale(1.7 1.3)\"",
                ),
        ),
        "toolbar-down" => ImageSrc::svg_markup(
            &include_str!("../../assets/icons/down.svg")
                .replace("0 0 24 24", "4 4 16 16")
                .replace("stroke-width=\"1.6\"", "stroke-width=\"2.1\""),
        ),
        "target-down" => ImageSrc::svg_markup(
            &include_str!("../../assets/icons/down.svg")
                .replace("0 0 24 24", "4.8 4.8 14.4 14.4")
                .replace("stroke-width=\"1.6\"", "stroke-width=\"1.32\""),
        ),
        "plot-select" => ImageSrc::svg_markup(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="m6 8 6-5 6 5m-12 8 6 5 6-5"/></svg>"#,
        ),
        "palette" => ImageSrc::svg_markup(include_str!("../../assets/icons/palette.svg")),
        "copy" => ImageSrc::svg_markup(include_str!("../../assets/icons/copy.svg")),
        "logs-copy-document" => ImageSrc::svg_markup(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M7.952 4.771 L7.952 2.169 C7.952 1.012,8.530 0.434,9.687 0.434 L14.313 0.434 L21.253 7.084 L21.253 15.759 C21.253 17.205,20.559 18.072,19.229 18.072 L17.494 18.072 M14.313 0.434 L14.313 7.084 L21.253 7.084 M5.060 5.060 L9.398 5.060 L16.627 12.289 L16.627 20.675 C16.627 21.947,16.106 22.699,14.834 22.699 L5.060 22.699 C3.846 22.699,3.325 21.947,3.325 20.675 L3.325 6.795 C3.325 5.639,3.904 5.060,5.060 5.060 Z M9.398 5.060 L9.398 12.289 L16.627 12.289"/></svg>"#,
        ),
        "logs-export" => ImageSrc::svg_markup(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M9.241 7.586 L7.586 7.586 C5.766 7.586,4.993 8.248,4.993 9.793 L4.993 17.793 C4.993 19.338,5.710 20.276,7.200 20.276 L16.966 20.276 C18.455 20.276,19.172 19.338,19.172 17.793 L19.172 9.793 C19.172 8.248,18.234 7.586,16.414 7.586 L14.759 7.586 M12.000 13.324 L12.000 1.683 M9.076 4.552 L12.000 1.683 L14.924 4.552"/></svg>"#,
        ),
        "logs-trash" | "trash-portrait" => ImageSrc::svg_markup(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M3.972 5.103 L19.476 5.103 M8.690 5.103 L8.690 2.731 C8.690 1.959,9.186 1.683,9.959 1.683 L13.490 1.683 C14.262 1.683,14.759 1.959,14.759 2.731 L14.759 5.103 M5.379 5.655 L6.207 18.621 C6.317 19.779,6.759 20.331,7.862 20.331 L16.138 20.331 C17.241 20.331,17.683 19.779,17.793 18.621 L18.621 5.655 M8.966 8.138 L8.966 17.297 M11.724 8.138 L11.724 17.297 M14.483 8.138 L14.483 17.297"/></svg>"#,
        ),
        "code" => ImageSrc::svg_markup(include_str!("../../assets/icons/code.svg")),
        "play" => ImageSrc::svg_markup(include_str!("../../assets/icons/play.svg")),
        "add-rule" => ImageSrc::svg_markup(include_str!("../../assets/icons/add-rule.svg")),
        "import" => ImageSrc::svg_markup(include_str!("../../assets/icons/import.svg")),
        "arrow-down" => ImageSrc::svg_markup(include_str!("../../assets/icons/arrow-down.svg")),
        "arrow-up" => ImageSrc::svg_markup(include_str!("../../assets/icons/arrow-up.svg")),
        "arrow-down-left" => {
            ImageSrc::svg_markup(include_str!("../../assets/icons/arrow-down-left.svg"))
        }
        "arrow-up-right" => {
            ImageSrc::svg_markup(include_str!("../../assets/icons/arrow-up-right.svg"))
        }
        "gear" => ImageSrc::svg_markup(include_str!("../../assets/icons/gear.svg")),
        "download-circle" => {
            ImageSrc::svg_markup(include_str!("../../assets/icons/download-circle.svg"))
        }
        "check" => ImageSrc::svg_markup(include_str!("../../assets/icons/check-bold.svg")),
        "sidebar" => ImageSrc::svg_markup(include_str!("../../assets/icons/sidebar.svg")),
        "composer" => ImageSrc::svg_markup(include_str!("../../assets/icons/composer.svg")),
        "bookmark" => ImageSrc::svg_markup(include_str!("../../assets/icons/bookmark.svg")),
        "logs" => ImageSrc::svg_markup(include_str!("../../assets/icons/logs.svg")),
        "logs-empty" => ImageSrc::svg_markup(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M3.273 2.182H21 M3.273 7.227H13.909 M3.273 12.273H21 M3.273 17.182H13.909"/></svg>"#,
        ),
        "logs-search-empty" => ImageSrc::svg_markup(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><circle cx="10.435" cy="7.565" r="6.522"/><path d="M15.046 12.177L20.296 17.426"/></svg>"#,
        ),
        "logs-no-errors" => ImageSrc::svg_markup(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="9.191" r="8.391"/><path d="M8.553 9.957L10.979 12.511L15.447 6.128"/></svg>"#,
        ),
        "memory" => ImageSrc::svg_markup(include_str!("../../assets/icons/memory.svg")),
        "search" => ImageSrc::svg_markup(include_str!("../../assets/icons/search.svg")),
        "auto-detect" => ImageSrc::svg_markup(include_str!("../../assets/icons/auto-detect.svg")),
        "filter" => ImageSrc::svg_markup(include_str!("../../assets/icons/filter.svg")),
        "up" => ImageSrc::svg_markup(include_str!("../../assets/icons/up.svg")),
        "down" => ImageSrc::svg_markup(include_str!("../../assets/icons/down.svg")),
        "right" => ImageSrc::svg_markup(include_str!("../../assets/icons/right.svg")),
        "refresh" => ImageSrc::svg_markup(include_str!("../../assets/icons/refresh.svg")),
        "file" => ImageSrc::svg_markup(include_str!("../../assets/icons/file.svg")),
        "drop-doc" => ImageSrc::svg_markup(include_str!("../../assets/icons/drop-doc.svg")),
        "folder" => ImageSrc::svg_markup(include_str!("../../assets/icons/folder.svg")),
        "more" | "memory-more" => {
            ImageSrc::svg_markup(include_str!("../../assets/icons/ellipsis-circle.svg"))
        }
        "settings" => ImageSrc::svg_markup(include_str!("../../assets/icons/settings.svg")),
        "link" => ImageSrc::svg_markup(include_str!("../../assets/icons/link.svg")),
        "erase" => ImageSrc::svg_markup(include_str!("../../assets/icons/erase.svg")),
        "bolt" => ImageSrc::svg_markup(include_str!("../../assets/icons/bolt.svg")),
        "shield" => ImageSrc::svg_markup(include_str!("../../assets/icons/shield.svg")),
        "close" => ImageSrc::svg_markup(include_str!("../../assets/icons/close.svg")),
        "cable" => ImageSrc::svg_markup(include_str!("../../assets/icons/cable.svg")),
        "serial-empty-cable" => ImageSrc::svg_markup(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><g transform="matrix(.884615385 0 0 1.025641026 1.5210 -.03846)"><rect x="10" y="1.5" width="4" height="3" rx=".8"/><rect x="8.5" y="5" width="7" height="9" rx="1.2"/><rect x="11.2" y="14" width="1.6" height="8.5" rx=".8"/></g></svg>"#,
        ),
        "terminal" => ImageSrc::svg_markup(include_str!("../../assets/icons/terminal.svg")),
        "plus" => ImageSrc::svg_markup(include_str!("../../assets/icons/plus.svg")),
        "pause" => ImageSrc::svg_markup(include_str!("../../assets/icons/pause.svg")),
        "export" => ImageSrc::svg_markup(include_str!("../../assets/icons/export.svg")),
        "trash" => ImageSrc::svg_markup(include_str!("../../assets/icons/trash.svg")),
        "table" => ImageSrc::svg_markup(include_str!("../../assets/icons/table.svg")),
        "terminal-table" => ImageSrc::svg_markup(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><rect x="2.5" y="5" width="19" height="14" rx="1.5"/><path d="M2.5 10h19M8.8 5v14M15.2 5v14"/></svg>"#,
        ),
        "terminal-pause" => ImageSrc::svg_markup(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"><path d="M9 5v14M15 5v14"/></svg>"#,
        ),
        "terminal-composer" => ImageSrc::svg_markup(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><rect x="2.5" y="5" width="19" height="14" rx="1.5"/><rect x="4.5" y="13" width="15" height="4" rx="0.5" fill="currentColor" stroke="none"/></svg>"#,
        ),
        "send" => ImageSrc::svg_markup(include_str!("../../assets/icons/paperplane.svg")),
        "history" => ImageSrc::svg_markup(include_str!("../../assets/icons/history.svg")),
        "repeat" => ImageSrc::svg_markup(include_str!("../../assets/icons/repeat.svg")),
        "stop" => ImageSrc::svg_markup(include_str!("../../assets/icons/stop.svg")),
        "antenna" => ImageSrc::svg_markup(include_str!("../../assets/icons/antenna.svg")),
        "plot" => ImageSrc::svg_markup(include_str!("../../assets/icons/plot.svg")),
        "plot-empty" => ImageSrc::svg_markup(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><g transform="matrix(0.987012987 0 0 0.902777778 -0.518890201 -1.710227273)"><path d="M3 3v18h19.1"/><g transform="translate(0 .9)"><path d="M4 12l5-3 5 4 7-5"/><circle cx="9" cy="9" r="1.05" fill="currentColor"/><circle cx="14" cy="13" r="1.05" fill="currentColor"/><circle cx="21" cy="8" r="1.05" fill="currentColor"/></g></g></svg>"#,
        ),
        "error-circle" => ImageSrc::svg_markup(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><path d="M12 7v6m0 4v.1"/></svg>"#,
        ),
        "arrow-down-to-line" => ImageSrc::svg_markup(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3v13m-5-5 5 5 5-5M5.6 21h12.8"/></svg>"#,
        ),
        "warning" => ImageSrc::svg_markup(include_str!("../../assets/icons/warning.svg")),
        "warning-fill" => ImageSrc::svg_markup(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor"><path fill-rule="evenodd" d="M10.27 3.98a2 2 0 0 1 3.46 0l9.1 15.77A1.5 1.5 0 0 1 21.53 22H2.47a1.5 1.5 0 0 1-1.3-2.25L10.27 3.98ZM11 9v6h2V9h-2Zm0 8v2h2v-2h-2Z"/></svg>"#,
        ),
        _ => ImageSrc::svg_markup(include_str!("../../assets/icons/info.svg")),
    }
}

pub(super) fn icon(name: &str) -> Svg {
    svg(source(name)).size(px(16.)).flex_shrink_0()
}
