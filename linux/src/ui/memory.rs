use super::*;
impl Workbench {
    pub(super) fn memory_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let owner = cx.entity().downgrade();
        let mut page = div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(40.))
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().flex_1().text_size(px(15.)).child("内存"))
                    .child(
                        native_button("memory-export")
                            .small()
                            .ghost()
                            .icon(icons::icon("export"))
                            .tooltip("导出为二进制")
                            .disabled(self.memory.is_none())
                            .on_click(cx.listener(|s, _, _, cx| {
                                if let Some(memory) = &s.memory {
                                    match model::decode_hex(&memory.data_hex) {
                                        Ok(bytes) => {
                                            s.export_bytes("ChipDock-memory.bin", bytes, cx)
                                        }
                                        Err(e) => s.report_error(e.to_string(), cx),
                                    }
                                }
                            })),
                    )
                    .child(
                        native_button("memory-more")
                            .small()
                            .ghost()
                            .icon(icons::icon("more-circle"))
                            .tooltip("芯片操作")
                            .dropdown_menu(move |m, _, _| {
                                let layout = owner.clone();
                                let erase = owner.clone();
                                let copy = owner.clone();
                                m.item(PopupMenuItem::new("查看内存映射").on_click(
                                    move |_, _, cx| {
                                        let _ = layout.update(cx, |s, cx| {
                                            s.submit(Command::Layout(s.snapshot(cx)), cx)
                                        });
                                    },
                                ))
                                .item(PopupMenuItem::new("复制数据").on_click(move |_, _, cx| {
                                    let _ = copy.update(cx, |s, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            s.memory_output.read(cx).value().to_string(),
                                        ))
                                    });
                                }))
                                .item(
                                    PopupMenuItem::new("擦除整个芯片…").on_click(
                                        move |_, _, cx| {
                                            let _ = erase.update(cx, |s, cx| {
                                                s.confirmation = Some(s.snapshot(cx));
                                                s.page = "flash";
                                                let _ = s
                                                    .main_window
                                                    .update(cx, |_, w, _| w.activate_window());
                                                cx.notify();
                                            });
                                        },
                                    ),
                                )
                            }),
                    ),
            )
            .child(
                div()
                    .px_4()
                    .pb_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div().flex_1().child(
                            Input::new(&self.fields["address"])
                                .small()
                                .aria_label("起始地址"),
                        ),
                    )
                    .child(
                        div().w(px(96.)).child(
                            self.popup(
                                "length",
                                [64, 128, 256, 512, 1024, 2048, 4096]
                                    .into_iter()
                                    .map(|n| (n.to_string(), format!("{n} B")))
                                    .collect(),
                                cx,
                            ),
                        ),
                    )
                    .child(
                        self.button("read", "读取", cx, |s, _, cx| {
                            s.submit(Command::Memory(s.snapshot(cx)), cx)
                        })
                        .icon(icons::icon("download-circle"))
                        .disabled(!self.target_connected(cx) || self.is_busy()),
                    ),
            );
        if !self.layout.is_empty() {
            page = page.child(
                div()
                    .id("memory-regions")
                    .max_h(px(140.))
                    .overflow_y_scroll()
                    .border_y_1()
                    .border_color(cx.theme().border)
                    .flex()
                    .flex_col()
                    .children(self.layout.iter().enumerate().map(|(i, r)| {
                        let start = r.start;
                        let length = r.size.min(4096);
                        native_button(("memory-region", i))
                            .small()
                            .ghost()
                            .label(format!(
                                "{:?}  0x{:08X} – 0x{:08X}  {} B",
                                r.kind, r.start, r.end, r.size
                            ))
                            .on_click(cx.listener(move |s, _, w, cx| {
                                s.set("address", format!("0x{start:X}"), w, cx);
                                s.set("length", length.to_string(), w, cx);
                            }))
                    })),
            )
        }
        if self.memory.is_some() {
            page = page.child(
                Textarea::new(&self.memory_output)
                    .font_family(cx.theme().mono_font_family.clone())
                    .appearance(false)
                    .flex_1()
                    .min_h_0()
                    .readonly(true),
            );
        } else {
            page = page.child(
                div()
                    .flex_1()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .text_color(cx.theme().muted_foreground)
                    .child(icons::icon("memory").size(px(38.)))
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("尚未读取"),
                    )
                    .child(if self.target_connected(cx) {
                        "输入地址后点击“读取”"
                    } else {
                        "连接目标后可读取内存"
                    }),
            );
        }
        page.into_any_element()
    }
}
