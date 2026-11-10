use super::*;

impl Workbench {
    pub(super) fn firmware_document(&self, margin: Pixels, cx: &mut Context<Self>) -> Div {
        let p = self.snapshot(cx);
        let path = std::path::Path::new(p.get("firmware"));
        let empty = p.get("firmware").is_empty();
        let mut document = div()
            .size_full()
            .flex()
            .flex_col()
            .gap(px(if empty { 22. } else { 20. }))
            .px(margin)
            .py_6();
        if empty {
            let busy = self.is_busy();
            document = document.child(
                div()
                    .relative()
                    .flex_1()
                    .min_h(px(170.))
                    .py(px(26.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(8.))
                    .child(dashed_outline(
                        cx.theme().foreground.opacity(0.175),
                        10.,
                        1.5,
                        [6., 4.],
                    ))
                    .child(
                        icons::icon("drop-doc")
                            .size(px(36.))
                            .mb(px(4.))
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
                            .text_size(px(12.))
                            .text_color(cx.theme().muted_foreground)
                            .child("支持 ELF、AXF、HEX 与 BIN 文件"),
                    )
                    .child(
                        div().mt(px(8.)).child(
                            self.button("pick-firmware", "选择文件…", cx, |s, _, cx| {
                                s.pick("firmware", cx)
                            })
                            .icon(icons::icon("folder")),
                        ),
                    ),
            );
            if !self.preferences.recent.is_empty() {
                let files = self.preferences.recent.iter().take(5).collect::<Vec<_>>();
                let missing = files
                    .iter()
                    .any(|path| !std::path::Path::new(path).is_file());
                let stripe = cx.theme().foreground.opacity(0.03);
                document = document.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(12.))
                                .child(
                                    div()
                                        .flex_1()
                                        .text_size(px(11.))
                                        .text_color(cx.theme().muted_foreground)
                                        .child("最近使用"),
                                )
                                .when(missing, |d| {
                                    d.child(
                                        borderless("recent-prune", "清除失效记录", cx)
                                            .disabled(busy)
                                            .on_click(cx.listener(|s, _, w, cx| {
                                                s.preferences
                                                    .recent
                                                    .retain(|p| std::path::Path::new(p).is_file());
                                                s.save(w, cx);
                                            })),
                                    )
                                })
                                .child(
                                    borderless("recent-clear", "清空列表", cx)
                                        .disabled(busy)
                                        .on_click(cx.listener(|s, _, w, cx| {
                                            s.preferences.recent.clear();
                                            s.save(w, cx);
                                        })),
                                ),
                        )
                        .child(
                            div()
                                .id("recent-firmware")
                                .max_h(px(152.))
                                .overflow_y_scroll()
                                .border_1()
                                .border_color(cx.theme().foreground.opacity(0.1))
                                .children(files.into_iter().enumerate().map(|(i, path)| {
                                    self.recent_row(i, path, i % 2 == 1, stripe, cx)
                                })),
                        ),
                );
            }
            return document;
        }
        let owner = cx.entity().downgrade();
        let busy = self.is_busy();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);
        let tint: Hsla = match ext.as_deref() {
            Some("elf" | "axf") => rgb(0xaf52de).into(),
            Some("bin") => rgb(0x30b0c7).into(),
            Some("hex" | "ihex") => rgb(0xff9500).into(),
            _ => cx.theme().muted_foreground,
        };
        let mono = cx.theme().mono_font_family.clone();
        let well = cx.theme().foreground.opacity(0.05);
        document = document.child(
            div()
                .flex()
                .items_center()
                .gap(px(14.))
                .child(
                    div()
                        .size(px(44.))
                        .flex_shrink_0()
                        .rounded(px(10.))
                        .bg(tint.opacity(0.14))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icons::icon("file").size(px(22.)).text_color(tint)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(3.))
                        .child(
                            div()
                                .text_size(px(17.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .truncate()
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
                                .truncate()
                                .child(abbreviate(path.parent().unwrap_or(path))),
                        ),
                )
                .child(
                    Button::new("firmware-menu")
                        .ghost()
                        .size(px(24.))
                        .p_0()
                        .rounded_full()
                        .text_color(cx.theme().foreground.opacity(0.85))
                        .child(icons::icon("ellipsis-circle").size(px(17.)).flex_shrink_0())
                        .tooltip("固件操作")
                        .disabled(busy)
                        .dropdown_menu(move |mut menu, _, _| {
                            for (action, label) in [(0, "更换固件…"), (1, REVEAL), (2, "关闭固件")]
                            {
                                if action == 2 {
                                    menu = menu.separator();
                                }
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
        let Some(preview) = &self.preview else {
            return document.child(if busy && self.operation.is_none() {
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(gpui_kit::component::spinner::Spinner::new().small())
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child("正在分析固件…"),
                    )
            } else {
                div()
                    .p(px(10.))
                    .rounded(px(6.))
                    .bg(rgba(0xff950014))
                    .flex()
                    .items_start()
                    .gap(px(8.))
                    .text_size(px(12.))
                    .child(
                        icons::icon("warning")
                            .size(px(14.))
                            .text_color(rgb(0xff9500)),
                    )
                    .child(div().flex_1().child(self.message.clone()))
            });
        };
        let u = &preview.usage;
        let used_start = u.start_address.unwrap_or(0);
        let used_end = u.end_address.unwrap_or(used_start);
        let flash = self
            .layout
            .iter()
            .filter(|r| matches!(r.kind, MemoryRegionKind::Nvm) && !r.is_alias)
            .filter(|r| u.start_address.is_none() || (r.start < used_end && r.end > used_start))
            .collect::<Vec<_>>();
        let start = flash
            .iter()
            .map(|r| r.start)
            .min()
            .unwrap_or(used_start)
            .min(used_start);
        let end = flash
            .iter()
            .map(|r| r.end)
            .max()
            .unwrap_or(used_end)
            .max(used_end.max(1));
        let capacity = flash.iter().map(|r| r.size).sum::<u64>();
        let span = (end - start).max(1) as f32;
        document = document.child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(
                    div()
                        .relative()
                        .h(px(8.))
                        .rounded(px(3.))
                        .overflow_hidden()
                        .bg(well)
                        .children(u.segments.iter().map(|segment| {
                            div()
                                .absolute()
                                .h_full()
                                .left(relative((segment.start - start) as f32 / span))
                                .w(relative(segment.size as f32 / span))
                                .min_w(px(2.))
                                .bg(cx.theme().primary)
                        })),
                )
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .text_size(px(10.))
                        .font_family(mono.clone())
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("0x{start:08X}"))
                        .when(capacity > 0, |d| {
                            d.child(
                                div()
                                    .when(u.used_bytes > capacity, |d| {
                                        d.text_color(cx.theme().danger)
                                    })
                                    .child(format!(
                                        "Flash {} · 已用 {:.1}%",
                                        byte_count(capacity),
                                        u.used_bytes as f64 / capacity as f64 * 100.
                                    )),
                            )
                        })
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
        let count = rows.len();
        document = document.child(
            div()
                .flex_1()
                .min_h(px(64.))
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .child(div().flex_1().text_size(px(12.)).child("固件内容"))
                        .child(
                            div()
                                .text_size(px(10.))
                                .font_family(mono.clone())
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
                        .border_color(cx.theme().foreground.opacity(0.12))
                        .rounded(px(6.))
                        .overflow_hidden()
                        .child(
                            div()
                                .px(px(12.))
                                .py(px(6.))
                                .flex()
                                .items_center()
                                .gap(px(12.))
                                .bg(well)
                                .border_b_1()
                                .border_color(cx.theme().foreground.opacity(0.1))
                                .text_size(px(10.))
                                .text_color(cx.theme().muted_foreground)
                                .child(div().w(px(80.)).child("地址"))
                                .child(div().w(px(376.)).child("十六进制"))
                                .child("ASCII"),
                        )
                        .child(
                            uniform_list("firmware-data", count, move |range, _, cx| {
                                range
                                    .map(|i| {
                                        let (address, hex, ascii) = &rows[i];
                                        div()
                                            .h(px(17.))
                                            .px(px(12.))
                                            .flex()
                                            .items_center()
                                            .gap(px(12.))
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
                            .py(px(4.))
                            .flex_1()
                            .min_h_0(),
                        ),
                ),
        );
        let fact = |label: &'static str, value: String, mono_value: bool| {
            div()
                .flex()
                .flex_col()
                .gap(px(3.))
                .child(
                    div()
                        .text_size(px(10.))
                        .text_color(cx.theme().muted_foreground)
                        .child(label),
                )
                .child(
                    div()
                        .when(mono_value, |d| d.font_family(mono.clone()))
                        .child(value),
                )
        };
        document.child(
            div()
                .flex()
                .items_start()
                .gap(px(36.))
                .child(fact(
                    "格式",
                    match u.format {
                        FirmwareFormat::Elf => "ELF",
                        FirmwareFormat::Hex => "HEX",
                        FirmwareFormat::Bin => "BIN",
                    }
                    .into(),
                    false,
                ))
                .child(fact("写入大小", byte_count(u.used_bytes), false))
                .child(fact("区段", u.segments.len().to_string(), false))
                .child(fact(
                    "地址范围",
                    format!("0x{used_start:08X} – 0x{:08X}", used_end.saturating_sub(1)),
                    true,
                )),
        )
    }
}

fn byte_count(value: u64) -> String {
    match value {
        1_048_576.. => format!("{:.1} MiB", value as f64 / 1_048_576.),
        1024.. => format!("{:.1} KiB", value as f64 / 1024.),
        _ => format!("{value} B"),
    }
}

impl Workbench {
    fn recent_row(
        &self,
        i: usize,
        path: &str,
        odd: bool,
        stripe: Hsla,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let file = std::path::Path::new(path);
        let exists = file.is_file();
        let selected = self.selected_recent.as_deref() == Some(path);
        let busy = self.is_busy();
        let tint = match file
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("elf" | "axf") => rgb(0xaf52de),
            Some("bin") => rgb(0x30b0c7),
            _ => rgb(0xff9500),
        };
        let used = self.preferences.recent_used.get(path).copied();
        let path = path.to_owned();
        let (open, remove, menu) = (path.clone(), path.clone(), path.clone());
        let secondary = if selected {
            gpui_kit::white().opacity(0.8)
        } else {
            cx.theme().muted_foreground
        };
        div()
            .id(("recent", i))
            .h(px(30.))
            .px(px(8.))
            .flex()
            .items_center()
            .gap(px(8.))
            .text_size(px(13.))
            .when(odd, |d| d.bg(stripe))
            .when(selected, |d| {
                d.bg(cx.theme().primary).text_color(gpui_kit::white())
            })
            .child(div().w(px(16.)).flex().justify_center().child(
                icons::icon("file").size(px(15.)).text_color(if selected {
                    gpui_kit::white()
                } else {
                    tint.into()
                }),
            ))
            .child(
                div()
                    .flex_shrink_0()
                    .when(!exists && !selected, |d| {
                        d.text_color(cx.theme().muted_foreground)
                    })
                    .child(
                        file.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_color(secondary)
                    .child(abbreviate(file.parent().unwrap_or(file))),
            )
            .child(if exists {
                div()
                    .flex_shrink_0()
                    .text_size(px(12.))
                    .text_color(secondary)
                    .child(used.map(relative_time).unwrap_or_default())
            } else {
                div()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_1()
                    .text_size(px(12.))
                    .text_color(rgb(0xff9500))
                    .child(icons::icon("warning").size(px(13.)))
                    .child("文件不存在")
            })
            .child(
                Button::new(("recent-remove", i))
                    .ghost()
                    .size(px(18.))
                    .p_0()
                    .text_color(secondary)
                    .child(icons::icon("close").size(px(15.)).flex_shrink_0())
                    .tooltip("从最近使用中移除")
                    .disabled(busy)
                    .on_click(cx.listener(move |s, _, w, cx| {
                        s.preferences.recent.retain(|p| p != &remove);
                        s.save(w, cx);
                    })),
            )
            .on_click(cx.listener(move |s, e: &ClickEvent, w, cx| {
                s.selected_recent = Some(open.clone());
                if e.click_count() == 2 && !s.is_busy() && std::path::Path::new(&open).is_file() {
                    s.open_recent(&open, w, cx);
                }
                cx.notify();
            }))
            .context_menu({
                let owner = cx.entity().downgrade();
                move |m, _, _| {
                    let (open, reveal, remove) = (owner.clone(), owner.clone(), owner.clone());
                    let (a, b, c) = (menu.clone(), menu.clone(), menu.clone());
                    let exists = std::path::Path::new(&menu).is_file();
                    m.item(PopupMenuItem::new("打开").disabled(!exists).on_click(
                        move |_, w, cx| {
                            let _ = open.update(cx, |s, cx| s.open_recent(&a, w, cx));
                        },
                    ))
                    .item(
                        PopupMenuItem::new(REVEAL)
                            .disabled(!exists)
                            .on_click(move |_, _, cx| {
                                let _ = reveal
                                    .update(cx, |_, cx| cx.reveal_path(std::path::Path::new(&b)));
                            }),
                    )
                    .separator()
                    .item(
                        PopupMenuItem::new("从最近使用中移除").on_click(move |_, w, cx| {
                            let _ = remove.update(cx, |s, cx| {
                                s.preferences.recent.retain(|p| p != &c);
                                s.save(w, cx);
                            });
                        }),
                    )
                }
            })
            .into_any_element()
    }
    fn open_recent(&mut self, path: &str, w: &mut Window, cx: &mut Context<Self>) {
        let base = self
            .preferences
            .recent_bases
            .get(path)
            .cloned()
            .unwrap_or_default();
        self.set("base", base, w, cx);
        self.set("firmware", path.to_owned(), w, cx);
        cx.notify();
    }
}

/// The platform's name for revealing a file in its file manager.
pub(super) const REVEAL: &str = if cfg!(target_os = "macos") {
    "在访达中显示"
} else if cfg!(windows) {
    "在资源管理器中显示"
} else {
    "在文件管理器中显示"
};

fn abbreviate(path: &std::path::Path) -> String {
    if let Some(home) = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        && let Ok(relative) = path.strip_prefix(home)
    {
        return format!("~/{}", relative.display());
    }
    path.to_string_lossy().into_owned()
}
