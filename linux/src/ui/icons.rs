use gpui_kit::component::Icon;

pub(super) fn icon(name: &'static str) -> Icon {
    Icon::default().path(format!("icons/{name}.svg"))
}
