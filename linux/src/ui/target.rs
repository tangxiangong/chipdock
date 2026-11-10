use super::*;

impl Workbench {
    fn restore_target(&mut self, before: &Preferences, w: &mut Window, cx: &mut Context<Self>) {
        for key in ["chip", "probe", "speed", "wire", "base"] {
            self.set(key, before.get(key).to_owned(), w, cx);
        }
        self.preferences
            .flags
            .insert("under_reset".into(), before.flag("under_reset"));
        cx.notify();
    }
    pub(super) fn open_target_setup(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let owner = cx.entity().downgrade();
        let before = self.snapshot(cx);
        if !w.has_active_dialog(cx) {
            self.focus_handle.focus(w, cx);
        }
        w.open_dialog(cx, move |d, w, cx| {
            let body = owner
                .update(cx, |s, cx| s.target_setup(&before, w, cx))
                .unwrap_or_else(|_| div().into_any_element());
            let owner = owner.clone();
            let before = before.clone();
            d.title("目标配置")
                .close_button(false)
                .w(px(480.))
                .rounded(px(16.))
                .child(body)
                .on_cancel(move |_, w, cx| {
                    let _ = owner.update(cx, |s, cx| s.restore_target(&before, w, cx));
                    true
                })
        });
    }
    fn target_setup(
        &self,
        before: &Preferences,
        _w: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let background = control_bg(cx);
        let group = || {
            div()
                .rounded_lg()
                .bg(background)
                .px_3()
                .py_1()
                .flex()
                .flex_col()
        };
        let row = |name: &'static str| {
            div()
                .min_h(px(42.))
                .flex()
                .items_center()
                .gap_3()
                .child(div().flex_1().child(name))
        };
        let p = self.snapshot(cx);
        let chip = if p.get("chip").is_empty() {
            "自动识别".into()
        } else {
            p.get("chip").to_owned()
        };
        let mut probes = vec![(String::new(), "自动选择探针".into())];
        probes.extend(self.probes.iter().map(|p| {
            (
                p.identifier.clone(),
                p.product.clone().unwrap_or_else(|| p.identifier.clone()),
            )
        }));
        let before = before.clone();
        let mut page = div()
            .flex()
            .flex_col()
            .gap_5()
            .text_size(px(13.))
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground)
                    .child(if self.target_connected(cx) {
                        "已识别目标"
                    } else {
                        "未连接"
                    }),
            )
            .child(
                div().flex().flex_col().gap_2().child("探针").child(
                    group().child(
                        row("调试探针")
                            .child(div().w(px(240.)).child(self.popup("probe", probes, cx)))
                            .child(
                                native_button("scan")
                                    .small()
                                    .ghost()
                                    .icon(icons::icon("refresh"))
                                    .tooltip("扫描探针")
                                    .on_click(
                                        cx.listener(|s, _, _, cx| s.submit(Command::Scan, cx)),
                                    ),
                            ),
                    ),
                ),
            )
            .child(
                div().flex().flex_col().gap_2().child("目标芯片").child(
                    group()
                        .child(
                            row("芯片")
                                .border_b_1()
                                .border_color(cx.theme().border)
                                .child(
                                    native_button("chip-picker")
                                        .small()
                                        .ghost()
                                        .label(chip)
                                        .child(icons::icon("select"))
                                        .on_click(cx.listener(|s, w, window, cx| {
                                            let _ = w;
                                            s.open_chip_picker(window, cx);
                                        })),
                                ),
                        )
                        .child(
                            row("协议")
                                .border_b_1()
                                .border_color(cx.theme().border)
                                .child(
                                    div()
                                        .flex()
                                        .rounded_md()
                                        .bg(cx.theme().background)
                                        .children(
                                            [("swd", "SWD"), ("jtag", "JTAG")].into_iter().map(
                                                |(value, label)| {
                                                    native_button(value)
                                                        .small()
                                                        .ghost()
                                                        .w(px(54.))
                                                        .label(label)
                                                        .selected(p.get("wire") == value)
                                                        .on_click(cx.listener(
                                                            move |s, _, w, cx| {
                                                                s.set("wire", value, w, cx);
                                                                cx.notify();
                                                            },
                                                        ))
                                                },
                                            ),
                                        ),
                                ),
                        )
                        .child(
                            row("速度")
                                .border_b_1()
                                .border_color(cx.theme().border)
                                .child(
                                    div().w(px(90.)).child(
                                        Input::new(&self.fields["speed"])
                                            .small()
                                            .appearance(false)
                                            .aria_label("调试速度"),
                                    ),
                                )
                                .child("kHz"),
                        )
                        .child(
                            row("复位下连接").child(
                                gpui_kit::component::switch::Switch::new("under-reset")
                                    .small()
                                    .checked(self.preferences.flag("under_reset"))
                                    .on_click(cx.listener(|s, v, _, cx| {
                                        s.preferences.flags.insert("under_reset".into(), *v);
                                        cx.notify();
                                    })),
                            ),
                        ),
                ),
            )
            .child(
                div().flex().flex_col().gap_2().child("固件地址").child(
                    group().child(
                        row("BIN 基地址").child(
                            div().w(px(220.)).child(
                                Input::new(&self.fields["base"])
                                    .small()
                                    .appearance(false)
                                    .aria_label("BIN 基地址"),
                            ),
                        ),
                    ),
                ),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground)
                    .child("留空自动识别 ESP BIN；其他 BIN 可手动输入。ELF / HEX 使用文件内地址。"),
            );
        if cfg!(windows) {
            page = page.child(self.button(
                "repair-driver",
                "检查／修复探针驱动",
                cx,
                |s, _, cx| s.submit(Command::RepairProbe(s.snapshot(cx).get("probe").into()), cx),
            ));
        }
        page.child(
            div()
                .flex()
                .justify_end()
                .gap_2()
                .child(
                    native_button("cancel-target")
                        .small()
                        .label("取消")
                        .bg(background)
                        .border_0()
                        .on_click(cx.listener(move |s, _, w, cx| {
                            s.restore_target(&before, w, cx);
                            w.close_dialog(cx);
                        })),
                )
                .child(
                    self.button("connect", "连接", cx, |s, w, cx| {
                        s.submit(Command::Connect(s.snapshot(cx)), cx);
                        w.close_dialog(cx);
                    })
                    .icon(icons::icon("link"))
                    .bg(background)
                    .border_0(),
                ),
        )
        .into_any_element()
    }
    fn open_chip_picker(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let owner = cx.entity().downgrade();
        if !w.has_active_dialog(cx) {
            self.focus_handle.focus(w, cx);
        }
        w.open_dialog(cx, move |d, _, cx| {
            let body =
                owner
                    .update(cx, |s, cx| {
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(div().flex_1().child(
                                        Input::new(&s.fields["chip"]).aria_label("搜索芯片"),
                                    ))
                                    .child(
                                        s.button("search", "搜索", cx, |s, _, cx| {
                                            s.submit(
                                                Command::Search(s.snapshot(cx).get("chip").into()),
                                                cx,
                                            )
                                        })
                                        .icon(icons::icon("search")),
                                    ),
                            )
                            .child(s.button("auto-target", "自动识别", cx, |s, w, cx| {
                                s.set("chip", "", w, cx);
                                w.close_dialog(cx);
                            }))
                            .child(
                                div()
                                    .id("chip-results")
                                    .max_h(px(260.))
                                    .overflow_y_scroll()
                                    .flex()
                                    .flex_col()
                                    .children(s.chips.iter().enumerate().map(|(i, chip)| {
                                        let chip = chip.clone();
                                        native_button(("chip-result", i))
                                            .small()
                                            .ghost()
                                            .label(chip.clone())
                                            .on_click(cx.listener(move |s, _, w, cx| {
                                                s.set("chip", chip.clone(), w, cx);
                                                w.close_dialog(cx);
                                            }))
                                    })),
                            )
                    })
                    .unwrap_or_else(|_| div());
            d.title("选择芯片").w(px(440.)).child(body)
        });
    }
}
