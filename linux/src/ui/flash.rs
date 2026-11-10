use super::*;

impl Workbench {
    pub(super) fn flash_page(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = self.snapshot(cx);
        let margin = (window.viewport_size().width * 0.05).max(px(32.));
        let document = self.firmware_document(margin, cx);
        let mut settings = div()
            .id("flash-settings")
            .size_full()
            .overflow_y_scroll()
            .px(margin)
            .py_5()
            .flex()
            .flex_col()
            .gap_4();
        settings = settings.child(
            div()
                .flex()
                .items_center()
                .gap_4()
                .child(
                    div()
                        .w(px(52.))
                        .text_right()
                        .text_color(cx.theme().muted_foreground)
                        .child("目标"),
                )
                .child(
                    div()
                        .flex_1()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(div().size(px(7.)).rounded_full().bg(
                                    if self.target_connected(cx) {
                                        rgb(0x34C759).into()
                                    } else {
                                        cx.theme().muted_foreground.opacity(0.4)
                                    },
                                ))
                                .child(div().font_weight(FontWeight::SEMIBOLD).child(
                                    if p.get("chip").is_empty() {
                                        "自动识别".to_owned()
                                    } else {
                                        p.get("chip").to_owned()
                                    },
                                ))
                                .child(div().text_color(cx.theme().muted_foreground).child(
                                    if self.target_connected(cx) {
                                        "已连接"
                                    } else {
                                        "未连接"
                                    },
                                )),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(cx.theme().muted_foreground)
                                .child(format!(
                                    "{} · {} kHz · {}",
                                    p.get("wire").to_uppercase(),
                                    p.get("speed"),
                                    if p.get("probe").is_empty() {
                                        "自动选择探针"
                                    } else {
                                        p.get("probe")
                                    }
                                )),
                        ),
                )
                .when(self.target_connected(cx), |v| {
                    v.child(
                        native_button("disconnect-target")
                            .small()
                            .ghost()
                            .icon(icons::icon("unlink"))
                            .tooltip("断开目标")
                            .disabled(self.is_busy())
                            .on_click(cx.listener(|s, _, w, cx| s.disconnect_target(w, cx))),
                    )
                })
                .child(
                    self.button("target-setup", "更改…", cx, |s, w, cx| {
                        s.open_target_setup(w, cx)
                    })
                    .icon(icons::icon("settings")),
                ),
        );
        settings = settings
            .child(
                div()
                    .flex()
                    .gap_4()
                    .items_start()
                    .child(
                        div()
                            .w(px(52.))
                            .flex_shrink_0()
                            .text_right()
                            .text_color(cx.theme().muted_foreground)
                            .child("擦除"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
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
                                        "full" => "擦除芯片中的全部 Flash",
                                        "skip" => "保留现有 Flash，不执行擦除",
                                        _ => "仅擦除固件覆盖的扇区",
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_4()
                    .items_start()
                    .child(
                        div()
                            .w(px(52.))
                            .flex_shrink_0()
                            .text_right()
                            .text_color(cx.theme().muted_foreground)
                            .child("完成后"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(self.flag("verify", "校验写入的数据", cx))
                            .child(self.flag("reset", "复位并运行程序", cx))
                            .child(self.flag("dry_run", "仅预检查，不写入芯片", cx)),
                    ),
            );
        if let Some(request) = &self.confirmation {
            let request = request.clone();
            settings = settings.child(
                div()
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().danger)
                    .child(format!(
                        "确认擦除 {} 的全部 Flash 数据？",
                        request.get("chip")
                    ))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .mt_2()
                            .child(self.button("cancel-erase", "取消", cx, |s, _, cx| {
                                s.confirmation = None;
                                cx.notify();
                            }))
                            .child(self.button(
                                "confirm-erase",
                                "确认擦除",
                                cx,
                                move |s, _, cx| {
                                    s.confirmation = None;
                                    s.submit(Command::Erase(request.clone()), cx);
                                },
                            )),
                    ),
            );
        }
        settings = settings.child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .flex_1()
                        .text_size(px(12.))
                        .text_color(cx.theme().muted_foreground)
                        .child(if self.target_connected(cx) {
                            if p.get("firmware").is_empty() {
                                "选择固件后即可烧录"
                            } else if cx.platform_info().os == OperatingSystem::Mac {
                                "按 ⌘↩ 开始"
                            } else {
                                "按 Ctrl+Enter 开始"
                            }
                        } else {
                            if cx.platform_info().os == OperatingSystem::Mac {
                                "连接后即可烧录（⌘K）"
                            } else {
                                "连接后即可烧录（Ctrl+K）"
                            }
                        }),
                )
                .child(
                    self.button("erase", "擦除芯片…", cx, |s, _, cx| {
                        s.confirmation = Some(s.snapshot(cx));
                        cx.notify();
                    })
                    .icon(icons::icon("erase"))
                    .disabled(
                        !self.target_connected(cx) || self.is_busy() || self.confirmation.is_some(),
                    ),
                )
                .child(
                    self.button(
                        "flash",
                        if self.target_connected(cx) {
                            "烧录固件"
                        } else {
                            "连接目标"
                        },
                        cx,
                        |s, _, cx| {
                            s.submit(
                                if s.target_connected(cx) {
                                    Command::Flash(s.snapshot(cx))
                                } else {
                                    Command::Connect(s.snapshot(cx))
                                },
                                cx,
                            )
                        },
                    )
                    .icon(icons::icon(if self.target_connected(cx) {
                        "send"
                    } else {
                        "link"
                    }))
                    .rounded_full()
                    .primary()
                    .disabled(
                        self.is_busy()
                            || self.confirmation.is_some()
                            || (self.target_connected(cx) && p.get("firmware").is_empty()),
                    ),
                ),
        );
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
}
