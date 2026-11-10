use super::*;

impl Workbench {
    pub(super) fn logs_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let query = self.fields["log_search"].read(cx).value().to_lowercase();
        let errors_only = self.preferences.flag("errors_only");
        let follow = self.preferences.flag("follow_logs");
        let entries = self
            .logs
            .iter()
            .filter(|s| s.to_lowercase().contains(&query))
            .filter(|s| !errors_only || s.contains("[错误]"))
            .cloned()
            .collect::<Vec<_>>();
        let copy = self.logs.iter().cloned().collect::<Vec<_>>().join("\n");
        let export = copy.clone().into_bytes();
        let errors = self.logs.iter().filter(|s| s.contains("[错误]")).count();
        let empty = self.logs.is_empty();
        let leading = if cx.platform_info().os == OperatingSystem::Mac {
            108.
        } else {
            16.
        };
        let toggled = rgb(if cx.theme().is_dark() {
            0x484848
        } else {
            0xe3e3e3
        });
        let tool = |id: &'static str, icon: &'static str, on: bool| {
            Button::new(id)
                .ghost()
                .size(px(30.))
                .p_0()
                .rounded_full()
                .when(on, |b| b.bg(toggled))
                .text_color(cx.theme().foreground.opacity(0.75))
                .child(icons::icon(icon).size(px(17.)).flex_shrink_0())
        };
        let body = if empty || entries.is_empty() {
            let (icon, title, detail) = if empty {
                (
                    "logs",
                    "暂无日志",
                    "连接、烧录和读取内存的过程会记录在这里。",
                )
            } else if query.is_empty() {
                ("check", "没有错误", "")
            } else {
                ("search", "无结果", "检查拼写或尝试其他搜索。")
            };
            div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .text_color(cx.theme().muted_foreground)
                .child(icons::icon(icon).size(px(40.)).mb(px(4.)))
                .child(
                    div()
                        .text_size(px(17.))
                        .font_weight(FontWeight::BOLD)
                        .child(title),
                )
                .when(!detail.is_empty(), |d| {
                    d.child(div().text_size(px(13.)).child(detail))
                })
                .into_any_element()
        } else {
            div()
                .id("logs-list")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .track_scroll(&self.logs_scroll)
                .px(px(16.))
                .py(px(10.))
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(px(12.))
                .line_height(relative(1.3))
                .children(entries.iter().map(|s| {
                    div()
                        .when(s.contains("[错误]"), |d| d.text_color(cx.theme().danger))
                        .child(s.clone())
                }))
                .into_any_element()
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                gpui_kit::component::TitleBar::new()
                    .h(px(52.))
                    .pl(px(leading))
                    .pr(px(9.))
                    .bg(cx.theme().background)
                    .border_0()
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .text_size(px(15.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(cx.theme().foreground.opacity(0.7))
                                            .child("活动日志"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format!(
                                                "{} 条日志 · {errors} 条错误",
                                                entries.len()
                                            )),
                                    ),
                            )
                            .child(
                                glass(cx)
                                    .pl(px(12.))
                                    .pr(px(3.))
                                    .gap(px(4.))
                                    .child(
                                        icons::icon("search")
                                            .size(px(16.))
                                            .flex_shrink_0()
                                            .text_color(cx.theme().foreground.opacity(0.7)),
                                    )
                                    .child(
                                        div().w(px(180.)).child(
                                            Input::new(&self.fields["log_search"])
                                                .appearance(false)
                                                .text_size(px(13.))
                                                .aria_label("筛选日志"),
                                        ),
                                    )
                                    .child(
                                        tool("only-errors", "warning", errors_only)
                                            .tooltip("仅显示错误")
                                            .on_click(cx.listener(|s, _, _, cx| {
                                                s.preferences.flags.insert(
                                                    "errors_only".into(),
                                                    !s.preferences.flag("errors_only"),
                                                );
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        tool("follow-logs", "arrow-down", follow)
                                            .tooltip("上滚时暂停跟随；点击继续显示最新日志")
                                            .on_click(cx.listener(|s, _, _, cx| {
                                                s.preferences.flags.insert(
                                                    "follow_logs".into(),
                                                    !s.preferences.flag("follow_logs"),
                                                );
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        tool("copy-logs", "copy", false)
                                            .tooltip("复制全部日志")
                                            .disabled(empty)
                                            .on_click(move |_, _, cx| {
                                                cx.write_to_clipboard(ClipboardItem::new_string(
                                                    copy.clone(),
                                                ))
                                            }),
                                    )
                                    .child(
                                        tool("export-logs", "export", false)
                                            .tooltip("导出日志…")
                                            .disabled(empty)
                                            .on_click(cx.listener(move |s, _, _, cx| {
                                                s.export_bytes("ChipDock.log", export.clone(), cx)
                                            })),
                                    )
                                    .child(
                                        tool("clear-logs", "trash", false)
                                            .tooltip("清空日志")
                                            .disabled(empty)
                                            .on_click(cx.listener(|s, _, _, cx| {
                                                s.logs.clear();
                                                s.unseen_errors = 0;
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    ),
            )
            .child(body)
            .into_any_element()
    }
}
