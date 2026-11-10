use super::*;

impl Workbench {
    pub(super) fn firmware_document(&self, margin: Pixels, cx: &mut Context<Self>) -> Div {
        let p = self.snapshot(cx);
        let path = std::path::Path::new(p.get("firmware"));
        let mut document = div()
            .size_full()
            .flex()
            .flex_col()
            .gap_5()
            .px(margin)
            .py_6();
        if p.get("firmware").is_empty() {
            document = document.child(
                div()
                    .flex_1()
                    .min_h(px(160.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .border_1()
                    .border_dashed()
                    .border_color(cx.theme().border)
                    .rounded_lg()
                    .child(
                        icons::icon("firmware")
                            .size(px(32.))
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::MEDIUM)
                            .child("将固件拖到这里"),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(cx.theme().muted_foreground)
                            .child("支持 ELF、AXF、HEX 与 BIN 文件"),
                    )
                    .child(
                        div().mt_2().child(
                            self.button("pick-firmware", "选择文件…", cx, |s, _, cx| {
                                s.pick("firmware", cx)
                            })
                            .icon(icons::icon("folder")),
                        ),
                    ),
            );
            if !self.preferences.recent.is_empty() {
                document = document
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_size(px(12.))
                            .text_color(cx.theme().muted_foreground)
                            .child("最近使用")
                            .child(
                                native_button("recent-clear")
                                    .small()
                                    .ghost()
                                    .label("清空列表")
                                    .on_click(cx.listener(|s, w, window, cx| {
                                        let _ = w;
                                        s.preferences.recent.clear();
                                        s.save(window, cx);
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .id("recent-firmware")
                            .max_h(px(150.))
                            .overflow_y_scroll()
                            .border_1()
                            .border_color(cx.theme().border)
                            .children(self.preferences.recent.iter().take(5).enumerate().map(
                                |(i, path)| {
                                    let file = std::path::Path::new(path);
                                    let path = path.clone();
                                    let base = self
                                        .preferences
                                        .recent_bases
                                        .get(&path)
                                        .cloned()
                                        .unwrap_or_default();
                                    div()
                                        .id(("recent", i))
                                        .h(px(30.))
                                        .px_2()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(icons::icon("file").text_color(rgb(0xf08a2c)))
                                        .child(
                                            file.file_name()
                                                .unwrap_or_default()
                                                .to_string_lossy()
                                                .into_owned(),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .overflow_hidden()
                                                .text_size(px(12.))
                                                .text_color(cx.theme().muted_foreground)
                                                .child(
                                                    file.parent()
                                                        .unwrap_or(file)
                                                        .to_string_lossy()
                                                        .into_owned(),
                                                ),
                                        )
                                        .on_click(cx.listener(move |s, e: &ClickEvent, w, cx| {
                                            if e.click_count() == 2 && !s.is_busy() {
                                                s.set("base", base.clone(), w, cx);
                                                s.set("firmware", path.clone(), w, cx);
                                                cx.notify();
                                            }
                                        }))
                                },
                            )),
                    );
            }
            return document;
        }
        let owner = cx.entity().downgrade();
        document = document.child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .size(px(44.))
                        .rounded_lg()
                        .bg(rgba(0xff950020))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icons::icon("file").size(px(24.)).text_color(rgb(0xf08a2c))),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_size(px(18.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .overflow_hidden()
                                .child(
                                    path.file_name()
                                        .unwrap_or_default()
                                        .to_string_lossy()
                                        .into_owned(),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(cx.theme().muted_foreground)
                                .overflow_hidden()
                                .child(abbreviate(path.parent().unwrap_or(path))),
                        ),
                )
                .child(
                    native_button("firmware-menu")
                        .small()
                        .ghost()
                        .icon(icons::icon("more"))
                        .tooltip("固件操作")
                        .dropdown_menu(move |mut menu, _, _| {
                            for (action, label) in
                                [(0, "更换固件…"), (1, "显示文件"), (2, "关闭固件")]
                            {
                                let owner = owner.clone();
                                menu = menu.item(PopupMenuItem::new(label).on_click(
                                    move |_, w, cx| {
                                        let _ = owner.update(cx, |s, cx| match action {
                                            0 => s.pick("firmware", cx),
                                            1 => cx.reveal_path(std::path::Path::new(
                                                s.snapshot(cx).get("firmware"),
                                            )),
                                            _ => {
                                                s.set("firmware", "", w, cx);
                                                s.preview = None;
                                                cx.notify();
                                            }
                                        });
                                    },
                                ));
                            }
                            menu
                        }),
                ),
        );
        if let Some(preview) = &self.preview {
            let u = &preview.usage;
            let start = u.start_address.unwrap_or(0);
            let end = u.end_address.unwrap_or(start);
            let span = (end - start).max(1) as f32;
            document = document.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .relative()
                            .h(px(8.))
                            .rounded_sm()
                            .overflow_hidden()
                            .bg(control_bg(cx))
                            .children(u.segments.iter().map(|segment| {
                                div()
                                    .absolute()
                                    .h_full()
                                    .left(relative((segment.start - start) as f32 / span))
                                    .w(relative(segment.size as f32 / span))
                                    .bg(rgb(0x007aff))
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .text_size(px(11.))
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("0x{start:08X}"))
                            .child(format!("0x{:08X}", end.saturating_sub(1))),
                    ),
            );
            let rows = preview
                .bytes
                .chunks(16)
                .enumerate()
                .map(|(i, bytes)| {
                    (
                        format!("0x{:08X}", preview.start_address + i as u64 * 16),
                        bytes
                            .iter()
                            .map(|b| b.map_or("··".into(), |v| format!("{v:02X}")))
                            .collect::<Vec<_>>()
                            .join(" "),
                        bytes
                            .iter()
                            .map(|b| b.filter(|b| (32..127).contains(b)).map_or('·', char::from))
                            .collect::<String>(),
                    )
                })
                .collect::<Vec<_>>();
            document = document.child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div().flex().justify_between().child("固件内容").child(
                            div()
                                .text_size(px(11.))
                                .font_family(cx.theme().mono_font_family.clone())
                                .text_color(cx.theme().muted_foreground)
                                .child(format!(
                                    "0x{:08X} – 0x{:08X}",
                                    preview.start_address,
                                    preview.start_address
                                        + preview.bytes.len().saturating_sub(1) as u64
                                )),
                        ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .flex_col()
                            .border_1()
                            .border_color(cx.theme().border)
                            .rounded_md()
                            .overflow_hidden()
                            .child(
                                div()
                                    .h(px(25.))
                                    .px_3()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .bg(control_bg(cx))
                                    .text_size(px(11.))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(div().w(px(80.)).child("地址"))
                                    .child(div().w(px(376.)).child("十六进制"))
                                    .child("ASCII"),
                            )
                            .child(
                                uniform_list("firmware-data", rows.len(), move |range, _, cx| {
                                    range
                                        .map(|i| {
                                            let (address, hex, ascii) = &rows[i];
                                            div()
                                                .h(px(17.))
                                                .px_3()
                                                .flex()
                                                .gap_3()
                                                .font_family(cx.theme().mono_font_family.clone())
                                                .text_size(px(12.))
                                                .child(
                                                    div()
                                                        .w(px(80.))
                                                        .text_color(cx.theme().muted_foreground)
                                                        .child(address.clone()),
                                                )
                                                .child(div().w(px(376.)).child(hex.clone()))
                                                .child(
                                                    div()
                                                        .text_color(cx.theme().muted_foreground)
                                                        .child(ascii.clone()),
                                                )
                                                .into_any_element()
                                        })
                                        .collect()
                                })
                                .flex_1()
                                .min_h_0(),
                            ),
                    ),
            );
            document = document.child(
                div().flex().gap_8().children(
                    [
                        ("格式", format!("{:?}", u.format).to_uppercase()),
                        (
                            "写入大小",
                            format!("{:.1} KiB", u.used_bytes as f64 / 1024.),
                        ),
                        ("区段", u.segments.len().to_string()),
                        (
                            "地址范围",
                            format!("0x{start:08X} – 0x{:08X}", end.saturating_sub(1)),
                        ),
                    ]
                    .into_iter()
                    .map(|(label, value)| {
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(label),
                            )
                            .child(value)
                    }),
                ),
            );
        } else {
            document = document.child(
                div()
                    .text_size(px(13.))
                    .text_color(cx.theme().muted_foreground)
                    .child(self.message.clone()),
            );
        }
        document
    }
}

fn abbreviate(path: &std::path::Path) -> String {
    if let Some(home) = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        && let Ok(relative) = path.strip_prefix(home)
    {
        return format!("~/{}", relative.display());
    }
    path.to_string_lossy().into_owned()
}
