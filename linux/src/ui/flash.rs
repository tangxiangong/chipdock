use super::*;

impl Workbench {
    pub(super) fn flash_page(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = self.snapshot(cx);
        let margin = (window.viewport_size().width * 0.05).max(px(32.));
        let document = self.firmware_document(margin, cx);
        let compact = window.viewport_size().height < px(560.);
        let connected = self.target_connected(cx);
        let busy = self.is_busy();
        let mac = cx.platform_info().os == OperatingSystem::Mac;
        let secondary = cx.theme().muted_foreground;
        let label = |text: &'static str| {
            div()
                .w(px(52.))
                .flex_shrink_0()
                .text_right()
                .text_color(secondary)
                .child(text)
        };
        let target = self.target_name(cx);
        let probe = self
            .probes
            .iter()
            .find(|probe| probe.identifier == p.get("probe"))
            .map(|probe| probe.product.clone().unwrap_or(probe.identifier.clone()))
            .unwrap_or_else(|| {
                if p.get("probe").is_empty() {
                    "自动选择探针".into()
                } else {
                    p.get("probe").into()
                }
            });
        let grid = div()
            .flex()
            .flex_col()
            .gap(px(if compact { 12. } else { 16. }))
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(14.))
                    .child(label("目标").mt(px(4.)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .mt(px(4.))
                            .flex()
                            .flex_col()
                            .gap(px(3.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .child(div().size(px(7.)).rounded_full().bg(if connected {
                                        rgb(0x34C759).into()
                                    } else {
                                        cx.theme().muted_foreground.opacity(0.4)
                                    }))
                                    .child(
                                        div()
                                            .min_w_0()
                                            .truncate()
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(target.clone()),
                                    )
                                    .when(!connected, |d| {
                                        d.child(
                                            div()
                                                .flex_shrink_0()
                                                .text_color(cx.theme().muted_foreground)
                                                .child("未连接"),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!(
                                        "{} · {} kHz · {probe}",
                                        p.get("wire").to_uppercase(),
                                        p.get("speed"),
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_shrink_0()
                            .gap(px(8.))
                            .when(connected, |d| {
                                d.child(
                                    bordered("disconnect-target", Some("xmark-circle"), "断开连接")
                                        .tooltip(if mac {
                                            "断开目标（⇧⌘K）"
                                        } else {
                                            "断开目标（Ctrl+Shift+K）"
                                        })
                                        .disabled(busy)
                                        .on_click(
                                            cx.listener(|s, _, w, cx| s.disconnect_target(w, cx)),
                                        ),
                                )
                            })
                            .child(
                                self.button("target-setup", "更改…", cx, |s, w, cx| {
                                    s.open_target_setup(w, cx)
                                })
                                .icon(icons::icon("settings")),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(14.))
                    .child(label("擦除").mt(px(4.)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(div().w(px(100.)).child(self.compact_choice(
                                "erase",
                                &["automatic", "full", "skip"],
                                cx,
                            )))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(match p.get("erase") {
                                        "full" => "写入前擦除整个 Flash，其他数据会丢失",
                                        "skip" => "不擦除，目标区域须已擦除",
                                        _ => "仅擦除固件覆盖的扇区",
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(14.))
                    .child(label("完成后"))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(7.))
                            .child(self.flag("verify", "校验写入的数据", cx))
                            .child(self.flag("reset", "复位并运行程序", cx))
                            .child(self.flag("dry_run", "仅预检查，不写入芯片", cx)),
                    ),
            );
        let dry_run = self.preferences.flag("dry_run");
        let title = match self.operation {
            Some("connect") => "正在连接…".to_owned(),
            Some("erase") => "正在擦除…".to_owned(),
            Some("dry_run") => "正在预检查…".to_owned(),
            Some(_) => "正在烧录…".to_owned(),
            None if !connected => "连接目标".to_owned(),
            None if dry_run => "开始预检查".to_owned(),
            None => format!("烧录到 {target}"),
        };
        let icon = match () {
            _ if busy => None,
            _ if !connected => Some("link"),
            _ if dry_run => Some("shield"),
            _ => Some("bolt"),
        };
        let hint = match () {
            _ if busy => "",
            _ if !connected && mac => "连接后即可烧录（⌘K）",
            _ if !connected => "连接后即可烧录（Ctrl+K）",
            _ if p.get("firmware").is_empty() => "选择固件后即可烧录",
            _ if mac => "按 ⌘↩ 开始",
            _ => "按 Ctrl+Enter 开始",
        };
        let action = div()
            .flex()
            .items_center()
            .gap(px(16.))
            .child(
                div()
                    .flex_1()
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground)
                    .child(hint),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .when(busy, |d| {
                        d.child(gpui_kit::component::spinner::Spinner::new().small())
                    })
                    .child(
                        bordered("erase", Some("erase"), "擦除芯片…")
                            .tooltip(if connected {
                                "擦除整个芯片，不烧录"
                            } else {
                                "连接后即可擦除"
                            })
                            .disabled(!connected || busy || self.installing)
                            .on_click(cx.listener(|s, _, w, cx| s.confirm_erase(w, cx))),
                    )
                    .child(
                        prominent("flash", icon, title.clone())
                            .tooltip(title)
                            .disabled(
                                busy || self.installing
                                    || (connected && p.get("firmware").is_empty()),
                            )
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.submit(
                                    if s.target_connected(cx) {
                                        Command::Flash(s.snapshot(cx))
                                    } else {
                                        Command::Connect(s.snapshot(cx))
                                    },
                                    cx,
                                )
                            })),
                    ),
            );
        let settings = div()
            .id("flash-settings")
            .size_full()
            .overflow_y_scroll()
            .px(margin)
            .py(px(if compact { 20. } else { 24. }))
            .flex()
            .flex_col()
            .gap(px(if compact { 20. } else { 24. }))
            .child(grid)
            .child(action);
        v_resizable("firmware-split")
            .child(
                resizable_panel()
                    .size_range(px(220.)..px(1400.))
                    .child(document),
            )
            .child(
                resizable_panel()
                    .size(px(300.))
                    .size_range(px(260.)..px(420.))
                    .child(settings),
            )
            .into_any_element()
    }
    /// The macOS confirmation dialog for erasing the whole chip.
    fn confirm_erase(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let request = self.snapshot(cx);
        let target = self.target_name(cx);
        self.confirmation = Some(request.clone());
        self.focus_handle.focus(w, cx);
        let owner = cx.entity().downgrade();
        cx.notify();
        w.open_dialog(cx, move |d, _, cx| {
            let (cancel, erase, dismiss) = (owner.clone(), owner.clone(), owner.clone());
            let request = request.clone();
            d.w(px(260.))
                .p(px(16.))
                .rounded(px(18.))
                .centered()
                .close_button(false)
                .child(alert_body(
                    format!("擦除 {target} 的全部 Flash？"),
                    "芯片中的固件与数据将被永久删除，此操作无法撤销。",
                    cx,
                ))
                .child(
                    div()
                        .mt(px(16.))
                        .flex()
                        .gap(px(8.))
                        .child(
                            native_button("cancel-erase")
                                .flex_1()
                                .h(px(28.))
                                .rounded_full()
                                .child(div().text_size(px(13.)).child("取消"))
                                .on_click(move |_, w, cx| {
                                    let _ = cancel.update(cx, |s, cx| {
                                        s.confirmation = None;
                                        cx.notify();
                                    });
                                    w.close_dialog(cx);
                                }),
                        )
                        .child(
                            native_button("confirm-erase")
                                .flex_1()
                                .h(px(28.))
                                .rounded_full()
                                .text_color(cx.theme().danger)
                                .child(div().text_size(px(13.)).child("擦除全部 Flash"))
                                .on_click(move |_, w, cx| {
                                    let request = request.clone();
                                    let _ = erase.update(cx, |s, cx| {
                                        s.confirmation = None;
                                        s.submit(Command::Erase(request), cx);
                                    });
                                    w.close_dialog(cx);
                                }),
                        ),
                )
                .on_cancel(move |_, _, cx| {
                    let _ = dismiss.update(cx, |s, cx| {
                        s.confirmation = None;
                        cx.notify();
                    });
                    true
                })
        });
    }
}

impl Workbench {
    /// The macOS failure alert for probe operations: the message as title, the
    /// detail and recovery advice as informative text.
    pub(super) fn present_failure(
        &mut self,
        title: String,
        message: String,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let owner = cx.entity().downgrade();
        w.open_dialog(cx, move |d, _, cx| {
            let logs = owner.clone();
            d.w(px(260.))
                .p(px(16.))
                .rounded(px(18.))
                .centered()
                .close_button(false)
                .child(alert_body(title.clone(), message.clone(), cx))
                .child(
                    div()
                        .mt(px(16.))
                        .flex()
                        .gap(px(8.))
                        .child(
                            native_button("failure-logs")
                                .flex_1()
                                .h(px(28.))
                                .rounded_full()
                                .child(div().text_size(px(13.)).child("查看日志"))
                                .on_click(move |_, w, cx| {
                                    w.close_dialog(cx);
                                    let _ = logs.update(cx, |s, cx| {
                                        if let Err(e) = s.open_panel(auxiliary::Panel::Logs, cx) {
                                            s.report_error(e.to_string(), cx);
                                        }
                                    });
                                }),
                        )
                        .child(
                            Button::new("failure-ok")
                                .primary()
                                .flex_1()
                                .h(px(28.))
                                .rounded_full()
                                .child(div().text_size(px(13.)).child("确定"))
                                .on_click(|_, w, cx| w.close_dialog(cx)),
                        ),
                )
        });
    }
}

/// The application icon, bold title and informative text of an `NSAlert`.
pub(super) fn alert_body(
    title: impl Into<SharedString>,
    message: impl Into<SharedString>,
    cx: &App,
) -> Div {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(10.))
        .text_center()
        .child(img("app-icon.png").size(px(64.)))
        .child(
            div()
                .text_size(px(13.))
                .font_weight(FontWeight::BOLD)
                .child(title.into()),
        )
        .child(
            div()
                .text_size(px(11.))
                .text_color(cx.theme().foreground.opacity(0.85))
                .child(message.into()),
        )
}
