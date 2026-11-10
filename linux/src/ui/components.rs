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
                        native_button(SharedString::from(format!("import-{key}")))
                            .small()
                            .label("导入 JSON…")
                            .disabled(self.is_busy())
                            .on_click(cx.listener(move |s, _, _, cx| s.import_editor(key, cx))),
                    )
                    .child(
                        native_button(SharedString::from(format!("export-{key}")))
                            .small()
                            .label("导出 JSON…")
                            .disabled(self.is_busy())
                            .on_click(cx.listener(move |s, _, _, cx| {
                                let bytes = s.editors[key].read(cx).value().as_bytes().to_vec();
                                s.export_bytes(&format!("ChipDock-{key}.json"), bytes, cx);
                            })),
                    )
                    .child(
                        native_button(SharedString::from(format!("save-{key}")))
                            .small()
                            .label("保存")
                            .on_click(cx.listener(move |s, _, w, cx| {
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
                            })),
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
                    .child(selected)
                    .child(icons::icon("select")),
            )
            .justify_between()
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
    pub(super) fn target_connected(&self, cx: &App) -> bool {
        self.connected_target
            .as_ref()
            .is_some_and(|key| *key == self.snapshot(cx).target_key())
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
        Checkbox::new(key)
            .label(label)
            .checked(self.preferences.flag(key))
            .disabled(
                self.installing
                    || self.confirmation.is_some()
                    || (key == "under_reset" && self.busy_lanes.contains(&Lane::Probe)),
            )
            .on_click(cx.listener(move |s, v, _, cx| {
                s.preferences.flags.insert(key.into(), *v);
                cx.notify();
            }))
            .into_any_element()
    }
    pub(super) fn button(
        &self,
        id: &'static str,
        label: &'static str,
        cx: &mut Context<Self>,
        f: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Button {
        native_button(id)
            .small()
            .text_size(px(13.))
            .font_weight(FontWeight::NORMAL)
            .secondary()
            .border_0()
            .label(label)
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

fn option_label(value: &str) -> &str {
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
        0x303030
    } else {
        0xeeeeee
    })
    .into()
}

pub(super) fn native_accent(cx: &mut App) {
    let theme = Theme::global_mut(cx);
    theme.primary = rgb(if theme.is_dark() { 0x0a84ff } else { 0x007aff }).into();
    theme.primary_foreground = rgb(0xffffff).into();
    theme.secondary = rgb(if theme.is_dark() { 0x303030 } else { 0xeeeeee }).into();
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
