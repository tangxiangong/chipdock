use super::*;

impl Workbench {
    pub(super) fn field(&self, key: &'static str) -> AnyElement {
        let label = field_label(key);
        div()
            .flex()
            .flex_col()
            .gap_1()
            .flex_shrink_0()
            .min_w_0()
            .child(div().text_sm().child(label))
            .child(
                Input::new(&self.fields[key])
                    .aria_label(label)
                    .small()
                    .disabled(
                        self.installing
                            || self.confirmation.is_some()
                            || (["chip", "probe", "speed", "base", "firmware"].contains(&key)
                                && self.busy_lanes.contains(&Lane::Probe))
                            || (["port", "baud", "bits", "stop", "parity", "flow"].contains(&key)
                                && self
                                    .sessions
                                    .iter()
                                    .find(|s| Some(s.id) == self.active)
                                    .and_then(|s| s.status.as_ref())
                                    .is_some_and(|s| s.connected || s.reconnecting)),
                    ),
            )
            .into_any_element()
    }
    pub(super) fn row(&self, keys: &[&'static str]) -> AnyElement {
        div()
            .flex()
            .gap_3()
            .w_full()
            .children(keys.iter().map(|key| self.field(key)))
            .into_any_element()
    }
    pub(super) fn editor(&self, key: &'static str, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                Textarea::new(&self.editors[key])
                    .h(px(180.))
                    .disabled(self.installing),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        bordered(
                            SharedString::from(format!("import-{key}")),
                            None,
                            "导入 JSON…",
                        )
                        .disabled(self.is_busy())
                        .on_click(cx.listener(move |s, _, _, cx| s.import_editor(key, cx))),
                    )
                    .child(
                        bordered(
                            SharedString::from(format!("export-{key}")),
                            None,
                            "导出 JSON…",
                        )
                        .disabled(self.is_busy())
                        .on_click(cx.listener(move |s, _, _, cx| {
                            let bytes = s.editors[key].read(cx).value().as_bytes().to_vec();
                            s.export_bytes(&format!("ChipDock-{key}.json"), bytes, cx);
                        })),
                    )
                    .child(
                        bordered(SharedString::from(format!("save-{key}")), None, "保存").on_click(
                            cx.listener(move |s, _, w, cx| {
                                if key == "colors" {
                                    let p = s.snapshot(cx);
                                    if let Err(e) = chipdock_gpui::presentation::Colors::new(
                                        p.get("colors"),
                                        p.get("tx_color"),
                                    ) {
                                        s.report_error(e.to_string(), cx);
                                        return;
                                    }
                                }
                                s.save(w, cx);
                            }),
                        ),
                    ),
            )
            .into_any_element()
    }
    pub(super) fn choice(
        &self,
        key: &'static str,
        values: &'static [&'static str],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let _ = window;
        self.select_field(
            key,
            values
                .iter()
                .map(|v| {
                    (
                        v.to_string(),
                        if key == "stop" && *v == "15" {
                            "1.5".into()
                        } else {
                            option_label(v).to_string()
                        },
                    )
                })
                .collect(),
            cx,
        )
    }
    pub(super) fn select_field(
        &self,
        key: &'static str,
        options: Vec<(String, String)>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .flex_shrink_0()
            .min_w_0()
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(field_label(key)),
            )
            .child(self.popup(key, options, cx))
            .into_any_element()
    }
    pub(super) fn compact_choice(
        &self,
        key: &'static str,
        values: &'static [&'static str],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.popup(
            key,
            values
                .iter()
                .map(|v| (v.to_string(), option_label(v).to_string()))
                .collect(),
            cx,
        )
    }
    pub(super) fn popup(
        &self,
        key: &'static str,
        options: Vec<(String, String)>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.popup_sized(key, options, false, cx)
    }
    /// `small` matches AppKit's small control size (20 pt, 12 pt text).
    pub(super) fn popup_sized(
        &self,
        key: &'static str,
        options: Vec<(String, String)>,
        small: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let current = self.fields[key].read(cx).value().to_string();
        let selected = options
            .iter()
            .find(|(value, _)| *value == current)
            .map(|(_, label)| label.clone())
            .unwrap_or_else(|| {
                if current.is_empty() {
                    "自动选择".into()
                } else {
                    current.clone()
                }
            });
        let owner = cx.entity().downgrade();
        let locked = (key == "ending"
            && self.fields["format"].read(cx).value().as_ref() == "hex"
            && !self.preferences.flag("hex_ending"))
            || (["wire", "probe"].contains(&key) && self.busy_lanes.contains(&Lane::Probe))
            || self.installing
            || self.confirmation.is_some()
            || (["port", "bits", "stop", "parity", "flow"].contains(&key)
                && self.serial_connected());
        native_button(SharedString::from(format!("select-{key}")))
            .small()
            .tooltip(selected.clone())
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(6.))
                    .text_size(px(if small { 12. } else { 13. }))
                    .when(!small, |d| d.font_weight(FontWeight::MEDIUM))
                    .child(div().min_w_0().truncate().child(selected))
                    .child(icons::icon("select").size(px(12.)).flex_shrink_0()),
            )
            .justify_between()
            .rounded(px(if small { 5. } else { 7. }))
            .px(px(if small { 8. } else { 10. }))
            .when(small, |b| b.h(px(20.)))
            .bg(control_bg(cx))
            .border_0()
            .w_full()
            .disabled(locked)
            .dropdown_menu(move |mut menu, _, _| {
                for (value, label) in &options {
                    let value = value.clone();
                    let owner = owner.clone();
                    menu = menu.item(
                        PopupMenuItem::new(label.clone())
                            .checked(value == current)
                            .on_click(move |_, w, cx| {
                                let _ = owner.update(cx, |s, cx| {
                                    s.set(key, value.clone(), w, cx);
                                    s.render_terminal(w, cx);
                                    cx.notify();
                                });
                            }),
                    );
                }
                menu
            })
            .into_any_element()
    }
    /// A sheet-style `.segmented` picker: an accent-filled selection with
    /// hairline separators between unselected neighbours.
    pub(super) fn segmented(
        &self,
        items: &[(&'static str, &'static str)],
        selected: &str,
        width: Option<f32>,
        cx: &mut Context<Self>,
        on_select: fn(&mut Self, &'static str, &mut Window, &mut Context<Self>),
    ) -> Div {
        let index = items.iter().position(|(key, _)| *key == selected);
        let rule = cx.theme().foreground.opacity(0.15);
        div()
            .flex()
            .items_center()
            .p(px(1.))
            .rounded(px(7.))
            .bg(control_bg(cx))
            .children(items.iter().enumerate().flat_map(|(i, (key, label))| {
                let key = *key;
                let on = Some(i) == index;
                let separator = i > 0 && !on && Some(i - 1) != index;
                [
                    separator.then(|| div().w(px(1.)).h(px(14.)).bg(rule).into_any_element()),
                    Some(
                        Button::new(SharedString::from(format!("segment-{key}")))
                            .ghost()
                            .map(|b| match width {
                                Some(w) => b.w(px(w)),
                                None => b.px(px(16.)),
                            })
                            .h(px(22.))
                            .rounded(px(6.))
                            .toggled(on)
                            .when(on, |b| {
                                b.bg(cx.theme().primary).text_color(gpui_kit::white())
                            })
                            .child(div().text_size(px(13.)).child(*label))
                            .on_click(cx.listener(move |s, _, w, cx| on_select(s, key, w, cx)))
                            .into_any_element(),
                    ),
                ]
                .into_iter()
                .flatten()
            }))
    }
    /// A grouped-form picker: trailing value with a round chevron badge.
    pub(super) fn form_popup(
        &self,
        key: &'static str,
        options: Vec<(String, String)>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let current = self.fields[key].read(cx).value().to_string();
        let label = options
            .iter()
            .find(|(v, _)| *v == current)
            .map(|(_, l)| l.clone())
            .unwrap_or_else(|| current.clone());
        let owner = cx.entity().downgrade();
        let locked = self.installing
            || (key == "ending"
                && self.fields["format"].read(cx).value().as_ref() == "hex"
                && !self.preferences.flag("hex_ending"));
        Button::new(SharedString::from(format!("form-{key}")))
            .ghost()
            .h(px(24.))
            .px(px(4.))
            .disabled(locked)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .text_size(px(13.))
                    .child(label)
                    .child(
                        div()
                            .size(px(20.))
                            .rounded_full()
                            .bg(control_bg(cx))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(icons::icon("select").size(px(12.)).flex_shrink_0()),
                    ),
            )
            .dropdown_menu(move |mut m, _, _| {
                for (value, label) in &options {
                    let value = value.clone();
                    let owner = owner.clone();
                    m = m.item(
                        PopupMenuItem::new(label.clone())
                            .checked(value == current)
                            .on_click(move |_, w, cx| {
                                let _ = owner.update(cx, |s, cx| {
                                    s.set(key, value.clone(), w, cx);
                                    s.render_terminal(w, cx);
                                    cx.notify();
                                });
                            }),
                    );
                }
                m
            })
            .into_any_element()
    }
    pub(super) fn target_connected(&self, cx: &App) -> bool {
        self.connected_target
            .as_ref()
            .is_some_and(|key| *key == self.snapshot(cx).target_key())
    }
    /// The identified device type, the connected chip, or the configured one.
    pub(super) fn target_name(&self, cx: &App) -> String {
        let p = self.snapshot(cx);
        match &self.target_info {
            Some(info) if self.target_connected(cx) => info.device_type.clone(),
            _ if p.get("chip").is_empty() => "自动识别".into(),
            _ => p.get("chip").into(),
        }
    }
    pub(super) fn serial_connected(&self) -> bool {
        self.sessions
            .iter()
            .find(|s| Some(s.id) == self.active)
            .and_then(|s| s.status.as_ref())
            .is_some_and(|s| s.connected || s.reconnecting)
    }
    pub(super) fn flag(
        &self,
        key: &'static str,
        label: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let checked = self.preferences.flag(key);
        let disabled = self.installing
            || self.confirmation.is_some()
            || (key == "under_reset" && self.busy_lanes.contains(&Lane::Probe));
        mac_checkbox(
            SharedString::from(format!("flag-{key}")),
            label,
            checked,
            disabled,
            cx,
        )
        .when(!disabled, |d| {
            d.on_click(cx.listener(move |s, _, _, cx| {
                s.preferences.flags.insert(key.into(), !checked);
                cx.notify();
            }))
        })
        .into_any_element()
    }
    pub(super) fn button(
        &self,
        id: &'static str,
        label: &'static str,
        cx: &mut Context<Self>,
        f: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Button {
        bordered(id, None, label)
            .disabled(
                (self.is_busy()
                    && !matches!(
                        id,
                        "memory-bin"
                            | "memory-copy"
                            | "plot-toggle"
                            | "plot-clear"
                            | "export-plot"
                            | "export-plot-csv"
                    ))
                    || self.installing
                    || (self.confirmation.is_some()
                        && !matches!(id, "confirm-erase" | "cancel-erase")),
            )
            .on_click(cx.listener(move |s, _, w, cx| f(s, w, cx)))
    }
}

pub(super) fn option_label(value: &str) -> &str {
    match value {
        "none" => "无",
        "automatic" => "按需擦除",
        "full" => "整片擦除",
        "skip" => "跳过擦除",
        "swd" => "SWD",
        "jtag" => "JTAG",
        "all" => "全部",
        "receive" => "接收",
        "send" => "发送",
        "odd" => "奇校验",
        "even" => "偶校验",
        "mark" => "Mark",
        "space" => "Space",
        "software" => "软件流控",
        "hardware" => "硬件流控",
        "hex" => "HEX",
        "utf-8" => "UTF-8",
        "ascii" => "ASCII",
        "gbk" => "GBK",
        "latin-1" => "Latin-1",
        "lf" => "LF",
        "cr" => "CR",
        "crlf" => "CRLF",
        "read" => "每次读取",
        "idle" => "空闲间隔",
        "header" => "帧头",
        "tail" => "帧尾",
        "fixed" => "固定长度",
        _ => value,
    }
}

fn field_label(key: &str) -> &str {
    match key {
        "stop" => "停止位",
        "port" => "串口设备",
        "flow" => "流控制",
        _ => model::FIELDS
            .iter()
            .find(|f| f.0 == key)
            .map_or(key, |f| f.1),
    }
}

pub(super) fn control_bg(cx: &App) -> Hsla {
    rgb(if cx.theme().is_dark() {
        0x3a3a3a
    } else {
        0xececec
    })
    .into()
}

pub(super) fn native_accent(cx: &mut App) {
    let theme = Theme::global_mut(cx);
    theme.primary = rgb(if theme.is_dark() { 0x0a84ff } else { 0x007aff }).into();
    theme.primary_foreground = rgb(0xffffff).into();
    theme.secondary = rgb(if theme.is_dark() { 0x3a3a3a } else { 0xececec }).into();
    theme.overlay = hsla(0., 0., 0., if theme.is_dark() { 0.35 } else { 0.18 });
    theme.muted_foreground = hsla(
        0.,
        0.,
        if theme.is_dark() { 1. } else { 0. },
        if theme.is_dark() { 0.55 } else { 0.5 },
    );
    theme.secondary_foreground = theme.foreground;
    theme.button_primary = theme.primary;
    theme.button_primary_hover = theme.primary;
    theme.button_primary_active = rgb(if theme.is_dark() { 0x006ee0 } else { 0x0063cc }).into();
    theme.button_primary_foreground = theme.primary_foreground;
    theme.button_secondary = theme.secondary;
    theme.button_secondary_foreground = theme.secondary_foreground;
    theme.tokens.primary = theme.primary.into();
    theme.tokens.button_primary = theme.primary.into();
    theme.tokens.button_primary_hover = theme.primary.into();
    theme.tokens.button_primary_active = theme.button_primary_active.into();
    theme.tokens.button_secondary = theme.secondary.into();
}

pub(super) fn native_button(id: impl Into<ElementId>) -> Button {
    Button::new(id)
        .secondary()
        .border_0()
        .text_size(px(13.))
        .font_weight(FontWeight::NORMAL)
}

/// AppKit's regular bordered push button: 24 pt high, 13 pt label.
pub(super) fn bordered(
    id: impl Into<ElementId>,
    icon: Option<&'static str>,
    label: impl Into<SharedString>,
) -> Button {
    let label = label.into();
    native_button(id)
        .with_size(gpui_kit::component::Size::Medium)
        .h(px(24.))
        .px(px(12.))
        .rounded(px(7.))
        .accessibility_label(label.clone())
        .when_some(icon, |b, icon| b.icon(icons::icon(icon)))
        .child(div().text_size(px(13.)).child(label))
}

/// A large `.borderedProminent` button: a 28 pt accent capsule.
pub(super) fn prominent(
    id: impl Into<ElementId>,
    icon: Option<&'static str>,
    label: impl Into<SharedString>,
) -> Button {
    let label = label.into();
    Button::new(id)
        .primary()
        .border_0()
        .with_size(gpui_kit::component::Size::Medium)
        .h(px(28.))
        .px(px(14.))
        .rounded(px(14.))
        .accessibility_label(label.clone())
        .when_some(icon, |b, icon| b.icon(icons::icon(icon)))
        .child(
            div()
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .child(label),
        )
}

/// A `.borderless` text button, tinted like AppKit's secondary label.
pub(super) fn borderless(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    cx: &App,
) -> Button {
    let label = label.into();
    Button::new(id)
        .ghost()
        .with_size(gpui_kit::component::Size::Small)
        .px_0()
        .h(px(18.))
        .text_color(cx.theme().muted_foreground)
        .accessibility_label(label.clone())
        .child(div().text_size(px(13.)).child(label))
}

/// The Liquid Glass capsule behind toolbar items.
pub(super) fn glass(cx: &App) -> Div {
    let dark = cx.theme().is_dark();
    div()
        .flex()
        .items_center()
        .flex_shrink_0()
        .h(px(36.))
        .rounded_full()
        .bg(rgb(if dark { 0x2b2b2b } else { 0xfcfcfc }))
        .border(px(0.5))
        .border_color(hsla(0., 0., if dark { 1. } else { 0. }, 0.13))
        .shadow(vec![BoxShadow {
            color: hsla(0., 0., 0., if dark { 0.35 } else { 0.05 }),
            offset: point(px(0.), px(1.)),
            blur_radius: px(4.),
            spread_radius: px(0.),
            inset: false,
        }])
}

/// A 38 × 36 pt icon button inside a toolbar glass capsule.
pub(super) fn toolbar_icon(id: impl Into<ElementId>, icon: &'static str, cx: &App) -> Button {
    Button::new(id)
        .ghost()
        .w(px(38.))
        .h(px(34.))
        .rounded_full()
        .text_color(cx.theme().foreground.opacity(0.85))
        .child(icons::icon(icon).size(px(18.)).flex_shrink_0())
}

/// AppKit's 16 pt checkbox with its label.
pub(super) fn mac_checkbox(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    checked: bool,
    disabled: bool,
    cx: &App,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(6.))
        .text_size(px(13.))
        .when(disabled, |d| d.opacity(0.45))
        .child(
            div()
                .size(px(16.))
                .flex_shrink_0()
                .rounded(px(4.))
                .flex()
                .items_center()
                .justify_center()
                .bg(if checked {
                    cx.theme().primary
                } else {
                    rgb(if cx.theme().is_dark() {
                        0x3c3c3c
                    } else {
                        0xe6e6e6
                    })
                    .into()
                })
                .when(checked, |b| {
                    b.child(
                        icons::icon("check-bold")
                            .size(px(11.))
                            .text_color(gpui_kit::white()),
                    )
                }),
        )
        .child(label.into())
}

/// SwiftUI's `strokeBorder(style: StrokeStyle(lineWidth:dash:))` over a rounded
/// rectangle. GPUI's own dashed borders derive the dash from the line width.
pub(super) fn dashed_outline(color: Hsla, radius: f32, width: f32, dash: [f32; 2]) -> Canvas<()> {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let inset = width / 2.;
            let (l, t) = (bounds.left() + px(inset), bounds.top() + px(inset));
            let (r, b) = (bounds.right() - px(inset), bounds.bottom() - px(inset));
            let k = px(radius - inset);
            let mut path = PathBuilder::stroke(px(width)).dash_array(&[px(dash[0]), px(dash[1])]);
            path.move_to(point(l + k, t));
            path.line_to(point(r - k, t));
            path.curve_to(point(r, t + k), point(r, t));
            path.line_to(point(r, b - k));
            path.curve_to(point(r - k, b), point(r, b));
            path.line_to(point(l + k, b));
            path.curve_to(point(l, b - k), point(l, b));
            path.line_to(point(l, t + k));
            path.curve_to(point(l + k, t), point(l, t));
            if let Ok(path) = path.build() {
                window.paint_path(path, color);
            }
        },
    )
    .absolute()
    .inset_0()
}

/// Foundation's named relative date style in Chinese ("2小时前").
pub(super) fn relative_time(seconds: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    match now.saturating_sub(seconds) {
        0..60 => "现在".into(),
        s @ 60..3600 => format!("{}分钟前", s / 60),
        s @ 3600..86400 => format!("{}小时前", s / 3600),
        86400..172800 => "昨天".into(),
        s @ 172800..604800 => format!("{}天前", s / 86400),
        604800..1209600 => "上周".into(),
        s @ 1209600..2592000 => format!("{}周前", s / 604800),
        s @ 2592000..31536000 => format!("{}个月前", s / 2592000),
        s => format!("{}年前", s / 31536000),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn relative_time_matches_named_presentation() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert_eq!(super::relative_time(now), "现在");
        assert_eq!(super::relative_time(now - 2 * 3600), "2小时前");
        assert_eq!(super::relative_time(now - 90_000), "昨天");
        assert_eq!(super::relative_time(now - 3 * 86400), "3天前");
    }
}
