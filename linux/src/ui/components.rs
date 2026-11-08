use super::*;

impl Workbench {
    pub(super) fn field(&self, key: &'static str) -> AnyElement {
        let label = model::FIELDS
            .iter()
            .find(|f| f.0 == key)
            .map_or(key, |f| f.1);
        div()
            .flex()
            .flex_col()
            .gap_1()
            .flex_1()
            .min_w(px(120.))
            .child(div().text_sm().child(label))
            .child(
                Input::new(&self.fields[key]).small().disabled(
                    self.confirmation.is_some()
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
                        Button::new(SharedString::from(format!("import-{key}")))
                            .small()
                            .label("导入 JSON…")
                            .disabled(self.is_busy())
                            .on_click(cx.listener(move |s, _, _, cx| s.import_editor(key, cx))),
                    )
                    .child(
                        Button::new(SharedString::from(format!("export-{key}")))
                            .small()
                            .label("导出 JSON…")
                            .disabled(self.is_busy())
                            .on_click(cx.listener(move |s, _, _, cx| {
                                let bytes = s.editors[key].read(cx).value().as_bytes().to_vec();
                                s.export_bytes(&format!("ChipDock-{key}.json"), bytes, cx);
                            })),
                    )
                    .child(
                        Button::new(SharedString::from(format!("save-{key}")))
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
        let current = self.fields[key].read(cx).value().to_string();
        let label = model::FIELDS
            .iter()
            .find(|f| f.0 == key)
            .map_or(key, |f| f.1);
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(div().text_sm().child(label))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .children(values.iter().map(|value| {
                        let value = *value;
                        Button::new(SharedString::from(format!("{key}-{value}")))
                            .small()
                            .label(value)
                            .selected(current == value)
                            .disabled(
                                self.installing
                                    || self.confirmation.is_some()
                                    || (["parity", "flow"].contains(&key)
                                        && self
                                            .sessions
                                            .iter()
                                            .find(|s| Some(s.id) == self.active)
                                            .and_then(|s| s.status.as_ref())
                                            .is_some_and(|s| s.connected || s.reconnecting)),
                            )
                            .on_click(cx.listener(move |s, _, w, cx| {
                                s.set(key, value, w, cx);
                                s.render_terminal(w, cx);
                                cx.notify();
                            }))
                    })),
            )
            .into_any_element()
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
            .disabled(self.installing || self.confirmation.is_some())
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
        Button::new(id)
            .small()
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
