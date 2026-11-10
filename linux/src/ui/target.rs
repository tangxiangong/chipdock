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
        let owner = cx.entity();
        let before = self.snapshot(cx);
        let sheet = cx.new(|cx| {
            cx.observe(&owner, |_, _, cx| cx.notify()).detach();
            TargetSheet {
                owner,
                before,
                picker: None,
            }
        });
        self.focus_handle.focus(w, cx);
        w.open_dialog(cx, move |d, _, cx| {
            let cancelling = sheet.clone();
            let confirming = sheet.clone();
            d.p_0()
                .centered()
                .close_button(false)
                .w(px(if sheet.read(cx).picker.is_some() {
                    500.
                } else {
                    480.
                }))
                .rounded(px(16.))
                .child(sheet.clone())
                .on_ok(move |_, w, cx| {
                    confirming.update(cx, |s, cx| s.confirm(w, cx));
                    false
                })
                .on_cancel(move |_, w, cx| {
                    cancelling.update(cx, |s, cx| {
                        if s.picker.take().is_some() {
                            cx.notify();
                            false
                        } else {
                            s.owner
                                .update(cx, |o, cx| o.restore_target(&s.before, w, cx));
                            true
                        }
                    })
                })
        });
    }
    fn target_setup(
        &self,
        before: &Preferences,
        _w: &mut Window,
        cx: &mut Context<Self>,
        sheet: WeakEntity<TargetSheet>,
    ) -> AnyElement {
        let background = rgb(if cx.theme().is_dark() {
            0x252525
        } else {
            0xf7f7f7
        });
        let separator = cx.theme().foreground.opacity(0.08);
        let secondary = cx.theme().muted_foreground;
        let mono = cx.theme().mono_font_family.clone();
        let group = || {
            div()
                .rounded(px(10.))
                .bg(background)
                .px(px(10.))
                .flex()
                .flex_col()
        };
        let row = |name: &'static str| {
            div()
                .min_h(px(37.))
                .flex()
                .items_center()
                .gap(px(8.))
                .child(div().flex_1().child(name))
        };
        let section = |title: &'static str| {
            div().flex().flex_col().gap(px(10.)).child(
                div()
                    .px(px(10.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
        };
        let footnote = || div().mt(px(6.)).px(px(10.)).text_size(px(10.));
        let p = self.snapshot(cx);
        let busy = self.is_busy();
        let connected = self.target_connected(cx);
        let chip = if p.get("chip").is_empty() {
            "自动识别".into()
        } else {
            p.get("chip").to_owned()
        };
        let mut probes = vec![(String::new(), "自动选择".into())];
        probes.extend(self.probes.iter().map(|p| {
            (
                p.identifier.clone(),
                p.product.clone().unwrap_or_else(|| p.identifier.clone()),
            )
        }));
        let before = before.clone();
        let speed_valid = p.get("speed").trim().parse::<u32>().is_ok_and(|v| v > 0);
        let base_valid = p.get("base").trim().is_empty() || model::number(p.get("base")).is_ok();
        let mut probe_section = section("探针").child(
            group().child(
                row("调试探针")
                    .h(px(42.))
                    .child(self.target_probe_menu(probes, cx))
                    .child(
                        bordered("scan", Some("refresh"), "")
                            .w(px(37.))
                            .px_0()
                            .accessibility_label("扫描探针")
                            .tooltip(if cfg!(target_os = "macos") {
                                "扫描调试探针（⌘R）"
                            } else {
                                "扫描调试探针（Ctrl+R）"
                            })
                            .disabled(busy)
                            .on_click(cx.listener(|s, _, _, cx| s.submit(Command::Scan, cx))),
                    ),
            ),
        );
        if self.has_scanned {
            probe_section = probe_section.child(if self.probes.is_empty() {
                footnote()
                    .text_color(rgb(0xff9500))
                    .child(if cfg!(target_os = "macos") {
                        "未发现调试探针。请检查 USB 数据线；使用 Apple 芯片的 Mac 笔记本时，请先解锁并允许此配件连接。关闭其他调试工具后重新扫描。"
                    } else {
                        "未发现调试探针。请检查 USB 数据线，关闭其他调试工具后重新扫描。"
                    })
            } else {
                footnote()
                    .text_color(secondary)
                    .child(format!("已发现 {} 个调试探针。", self.probes.len()))
            });
        }
        let mut page = div()
            .flex()
            .flex_col()
            .text_size(px(13.))
            .line_height(relative(1.2))
            .child(
                div()
                    .px(px(20.))
                    .pt(px(20.))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(div().font_weight(FontWeight::BOLD).child("目标配置"))
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(if connected {
                                rgb(0x34c759).into()
                            } else {
                                secondary
                            })
                            .child(if connected {
                                format!("已连接 {}", self.target_name(cx))
                            } else {
                                "未连接".into()
                            }),
                    ),
            );
        let mut chip_group = group()
            .child(
                row("芯片").border_b_1().border_color(separator).child(
                    Button::new("chip-picker")
                        .ghost()
                        .px_0()
                        .h(px(22.))
                        .text_color(secondary)
                        .disabled(busy)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.))
                                .child(div().font_family(mono.clone()).child(chip))
                                .child(icons::icon("select").size(px(11.))),
                        )
                        .on_click(move |_, w, cx| {
                            let _ = sheet.update(cx, |s, cx| s.select_chip(w, cx));
                        }),
                ),
            )
            .child(
                row("协议").border_b_1().border_color(separator).child(
                    div()
                        .flex()
                        .p(px(2.))
                        .rounded(px(7.))
                        .bg(control_bg(cx))
                        .children([("swd", "SWD"), ("jtag", "JTAG")].into_iter().map(
                            |(value, label)| {
                                let selected = p.get("wire") == value;
                                Button::new(value)
                                    .ghost()
                                    .h(px(24.))
                                    .w(px(54.))
                                    .rounded(px(6.))
                                    .disabled(busy)
                                    .when(selected, |b| {
                                        b.bg(rgb(if cx.theme().is_dark() {
                                            0x5a5a5a
                                        } else {
                                            0xd6d6d6
                                        }))
                                    })
                                    .child(div().text_size(px(13.)).child(label))
                                    .on_click(cx.listener(move |s, _, w, cx| {
                                        s.set("wire", value, w, cx);
                                        cx.notify();
                                    }))
                            },
                        )),
                ),
            )
            .child(
                row("速度")
                    .gap(px(6.))
                    .border_b_1()
                    .border_color(separator)
                    .child(
                        div().w(px(80.)).child(
                            Input::new(&self.fields["speed"])
                                .small()
                                .appearance(false)
                                .text_right()
                                .disabled(busy)
                                .aria_label("调试速度"),
                        ),
                    )
                    .child(div().text_color(secondary).child("kHz"))
                    .child(self.speed_menu(cx)),
            );
        if !speed_valid {
            chip_group = chip_group.child(
                div()
                    .min_h(px(30.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .border_b_1()
                    .border_color(separator)
                    .text_size(px(10.))
                    .text_color(cx.theme().danger)
                    .child(icons::icon("warning").size(px(12.)))
                    .child("速度须为 1–4294967295 kHz 的整数"),
            );
        }
        chip_group = chip_group.child(
            row("复位下连接").child(
                gpui_kit::component::switch::Switch::new("under-reset")
                    .small()
                    .disabled(busy)
                    .checked(self.preferences.flag("under_reset"))
                    .on_click(cx.listener(|s, v, _, cx| {
                        s.preferences.flags.insert("under_reset".into(), *v);
                        cx.notify();
                    })),
            ),
        );
        let mut base_group = group().child(
            row("BIN 基地址").child(
                div().w(px(220.)).child(
                    Input::new(&self.fields["base"])
                        .small()
                        .appearance(false)
                        .text_right()
                        .font_family(mono.clone())
                        .disabled(busy)
                        .aria_label("BIN 基地址"),
                ),
            ),
        );
        if !base_valid {
            base_group = base_group.child(
                div()
                    .min_h(px(30.))
                    .flex()
                    .items_center()
                    .border_t_1()
                    .border_color(separator)
                    .text_color(cx.theme().danger)
                    .child("请输入有效的十六进制地址"),
            );
        }
        page = page.child(
            div()
                .px(px(20.))
                .pt(px(22.))
                .flex()
                .flex_col()
                .gap(px(32.))
                .child(probe_section)
                .child(section("目标芯片").child(chip_group))
                .child(section("固件地址").child(base_group).child(
                    footnote().text_size(px(10.)).text_color(secondary).child(
                        "留空自动识别 ESP BIN；其他 BIN 可手动输入。ELF / HEX 使用文件内地址。",
                    ),
                ))
                .when_some(
                    self.target_info.as_ref().filter(|_| connected),
                    |d, info| {
                        let mut identified = group();
                        let mut rows = vec![("型号", info.device_type.clone())];
                        rows.extend(info.cpu.clone().map(|cpu| ("内核", cpu)));
                        rows.extend(
                            info.flash_size_bytes
                                .map(|bytes| ("Flash", format!("{:.1} KiB", bytes as f64 / 1024.))),
                        );
                        let last = rows.len() - 1;
                        for (i, (name, value)) in rows.into_iter().enumerate() {
                            identified = identified.child(
                                row(name)
                                    .when(i < last, |r| r.border_b_1().border_color(separator))
                                    .child(div().text_color(secondary).child(value)),
                            );
                        }
                        d.child(section("已识别").child(identified))
                    },
                ),
        );
        if cfg!(windows) {
            page = page.child(div().px(px(30.)).pt(px(16.)).child(self.button(
                "repair-driver",
                "检查／修复探针驱动",
                cx,
                |s, _, cx| s.submit(Command::RepairProbe(s.snapshot(cx).get("probe").into()), cx),
            )));
        }
        page.child(
            div()
                .px(px(24.))
                .pt(px(28.))
                .pb(px(20.))
                .flex()
                .items_center()
                .gap(px(8.))
                .when(connected, |d| {
                    d.child(
                        bordered("disconnect-sheet", Some("xmark-circle"), "断开")
                            .text_color(cx.theme().danger)
                            .disabled(busy)
                            .on_click(cx.listener(|s, _, w, cx| s.disconnect_target(w, cx))),
                    )
                })
                .child(div().flex_1())
                .when(busy, |d| {
                    d.child(
                        div()
                            .mr(px(4.))
                            .child(gpui_kit::component::spinner::Spinner::new().small()),
                    )
                })
                .child(
                    bordered("cancel-target", None, "取消")
                        .disabled(busy)
                        .on_click(cx.listener(move |s, _, w, cx| {
                            s.restore_target(&before, w, cx);
                            w.close_dialog(cx);
                        })),
                )
                .child(
                    bordered(
                        "connect",
                        Some("link"),
                        if connected { "重新连接" } else { "连接" },
                    )
                    .disabled(!self.can_connect_target(cx))
                    .on_click(cx.listener(|s, _, w, cx| {
                        s.submit(Command::Connect(s.snapshot(cx)), cx);
                        w.close_dialog(cx);
                    })),
                ),
        )
        .into_any_element()
    }
    fn can_connect_target(&self, cx: &App) -> bool {
        let p = self.snapshot(cx);
        !self.is_busy()
            && (!self.has_scanned || !self.probes.is_empty())
            && p.target().is_ok()
            && (p.get("base").trim().is_empty() || model::number(p.get("base")).is_ok())
    }
    fn target_probe_menu(
        &self,
        options: Vec<(String, String)>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let current = self.fields["probe"].read(cx).value().to_string();
        let label = options
            .iter()
            .find(|(v, _)| *v == current)
            .map(|(_, l)| l.clone())
            .unwrap_or_else(|| "自动选择".into());
        let owner = cx.entity().downgrade();
        Button::new("target-probe-menu")
            .ghost()
            .h(px(24.))
            .px(px(4.))
            .disabled(self.busy_lanes.contains(&Lane::Probe))
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
                            .child(icons::icon("select").size(px(12.))),
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
                                    s.set("probe", value.clone(), w, cx);
                                    cx.notify();
                                });
                            }),
                    );
                }
                m
            })
    }
    fn speed_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        native_button("speed-presets")
            .small()
            .ghost()
            .icon(icons::icon("down"))
            .tooltip("常用连接速度")
            .dropdown_menu(move |mut m, _, _| {
                for (speed, label) in [
                    (100, "100 kHz"),
                    (1000, "1,000 kHz"),
                    (4000, "4,000 kHz"),
                    (10000, "10,000 kHz"),
                    (24000, "24,000 kHz"),
                ] {
                    let owner = owner.clone();
                    m = m.item(PopupMenuItem::new(label).on_click(move |_, w, cx| {
                        let _ = owner.update(cx, |s, cx| {
                            s.set("speed", speed.to_string(), w, cx);
                            cx.notify();
                        });
                    }));
                }
                m
            })
    }
}

struct TargetSheet {
    owner: Entity<Workbench>,
    before: Preferences,
    picker: Option<Entity<super::chip_picker::ChipPicker>>,
}
impl TargetSheet {
    fn select_chip(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let initial = self.owner.read(cx).snapshot(cx).get("chip").to_owned();
        let core = self.owner.read(cx).worker.core.clone();
        let parent = cx.entity().downgrade();
        self.picker = Some(cx.new(|cx| {
            super::chip_picker::ChipPicker::new(initial, core, w, cx, move |selection, w, cx| {
                let _ = parent.update(cx, |s, cx| {
                    if let Some(chip) = selection {
                        s.owner.update(cx, |o, cx| o.set("chip", chip, w, cx));
                    }
                    s.picker = None;
                    cx.notify();
                });
            })
        }));
        cx.notify();
    }
    fn confirm(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if let Some(picker) = &self.picker {
            let picker = picker.clone();
            w.defer(cx, move |w, cx| picker.update(cx, |p, cx| p.confirm(w, cx)));
        } else {
            self.owner.update(cx, |o, cx| {
                let p = o.snapshot(cx);
                if o.can_connect_target(cx) {
                    o.submit(Command::Connect(p), cx);
                    w.close_dialog(cx);
                }
            });
        }
    }
}
impl Render for TargetSheet {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(picker) = &self.picker {
            return div().child(picker.clone());
        }
        let sheet = cx.entity().downgrade();
        div().child(
            self.owner
                .update(cx, |o, cx| o.target_setup(&self.before, w, cx, sheet)),
        )
    }
}
