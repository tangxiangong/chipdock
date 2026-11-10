use super::*;

impl Workbench {
    pub(super) fn logs_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let query = self.fields["log_search"].read(cx).value().to_lowercase();
        let entries = self
            .logs
            .iter()
            .filter(|s| s.to_lowercase().contains(&query))
            .filter(|s| !self.preferences.flag("errors_only") || s.contains("[错误]"))
            .cloned()
            .collect::<Vec<_>>();
        let copy = entries.join("\n");
        let export = copy.clone().into_bytes();
        let errors = self.logs.iter().filter(|s| s.contains("[错误]")).count();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                gpui_kit::component::TitleBar::new()
                    .h(px(52.))
                    .pr_3()
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .child(
                                        div().font_weight(FontWeight::SEMIBOLD).child("活动日志"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format!(
                                                "{} 条日志 · {errors} 条错误",
                                                self.logs.len()
                                            )),
                                    ),
                            )
                            .child(
                                div().w(px(180.)).child(
                                    Input::new(&self.fields["log_search"])
                                        .small()
                                        .aria_label("筛选日志")
                                        .prefix(icons::icon("search")),
                                ),
                            )
                            .child(
                                native_button("only-errors")
                                    .small()
                                    .ghost()
                                    .icon(icons::icon("info"))
                                    .tooltip("仅显示错误")
                                    .selected(self.preferences.flag("errors_only"))
                                    .on_click(cx.listener(|s, _, _, cx| {
                                        s.preferences.flags.insert(
                                            "errors_only".into(),
                                            !s.preferences.flag("errors_only"),
                                        );
                                        cx.notify();
                                    })),
                            )
                            .child(
                                native_button("follow-logs")
                                    .small()
                                    .ghost()
                                    .icon(icons::icon("arrow-down"))
                                    .tooltip("跟随最新日志")
                                    .selected(self.preferences.flag("follow_logs"))
                                    .on_click(cx.listener(|s, _, _, cx| {
                                        s.preferences.flags.insert(
                                            "follow_logs".into(),
                                            !s.preferences.flag("follow_logs"),
                                        );
                                        cx.notify();
                                    })),
                            )
                            .child(
                                native_button("copy-logs")
                                    .small()
                                    .ghost()
                                    .icon(icons::icon("copy"))
                                    .tooltip("复制全部日志")
                                    .on_click(move |_, _, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            copy.clone(),
                                        ))
                                    }),
                            )
                            .child(
                                native_button("export-logs")
                                    .small()
                                    .ghost()
                                    .icon(icons::icon("export"))
                                    .tooltip("导出日志…")
                                    .on_click(cx.listener(move |s, _, _, cx| {
                                        s.export_bytes("ChipDock.log", export.clone(), cx)
                                    })),
                            )
                            .child(
                                native_button("clear-logs")
                                    .small()
                                    .ghost()
                                    .icon(icons::icon("trash"))
                                    .tooltip("清空日志")
                                    .on_click(cx.listener(|s, _, _, cx| {
                                        s.logs.clear();
                                        s.unseen_errors = 0;
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .id("logs-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.logs_scroll)
                    .px_5()
                    .py_2()
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_size(px(12.))
                    .children(entries.into_iter().map(|s| div().child(s))),
            )
            .into_any_element()
    }
}
