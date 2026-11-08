use super::*;

impl Workbench {
    pub(super) fn flash_page(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let mut page = div()
            .flex()
            .flex_col()
            .gap_4()
            .child(div().text_xl().child("固件烧录"))
            .child(self.row(&["firmware", "base"]))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.button("pick-firmware", "选择固件…", cx, |s, _, cx| {
                        s.pick("firmware", cx)
                    }))
                    .child(self.button("preview", "分析固件", cx, |s, _, cx| {
                        s.submit(Command::Preview(s.snapshot(cx)), cx)
                    })),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        self.button("reveal-firmware", "显示固件文件", cx, |s, _, cx| {
                            let p = s.snapshot(cx);
                            if !p.get("firmware").is_empty() {
                                cx.reveal_path(std::path::Path::new(p.get("firmware")));
                            }
                        }),
                    )
                    .child(self.button("clear-firmware", "关闭固件", cx, |s, w, cx| {
                        s.set("firmware", "", w, cx);
                        s.preview = None;
                        s.firmware_output.update(cx, |s, cx| s.set_value("", w, cx));
                        cx.notify();
                    })),
            )
            .child(self.row(&["chip", "probe", "speed"]))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.button("scan", "刷新探针", cx, |s, _, cx| {
                        s.submit(Command::Scan, cx)
                    }))
                    .child(self.button("search", "搜索芯片", cx, |s, _, cx| {
                        s.submit(Command::Search(s.snapshot(cx).get("chip").into()), cx)
                    }))
                    .child(self.button("connect", "连接识别", cx, |s, _, cx| {
                        s.submit(Command::Connect(s.snapshot(cx)), cx)
                    }))
                    .child(
                        self.button("disconnect-target", "断开目标", cx, |s, w, cx| {
                            s.disconnect_target(w, cx)
                        }),
                    )
                    .child(self.button("layout", "读取芯片布局", cx, |s, _, cx| {
                        s.submit(Command::Layout(s.snapshot(cx)), cx)
                    })),
            )
            .child(self.choice("wire", &["swd", "jtag"], window, cx))
            .child(self.choice("erase", &["automatic", "full", "skip"], window, cx))
            .child(
                div()
                    .flex()
                    .gap_4()
                    .child(self.flag("under_reset", "复位下连接", cx))
                    .child(self.flag("verify", "写入后校验", cx))
                    .child(self.flag("reset", "完成后复位", cx))
                    .child(self.flag("dry_run", "仅预检查", cx)),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        self.button("flash", "烧录固件", cx, |s, _, cx| {
                            s.submit(Command::Flash(s.snapshot(cx)), cx)
                        })
                        .primary(),
                    )
                    .child(self.button("erase", "整片擦除…", cx, |s, _, cx| {
                        s.confirmation = Some(s.snapshot(cx));
                        cx.notify();
                    })),
            );
        if cfg!(windows) {
            page = page.child(self.button(
                "repair-driver",
                "检查／修复所选探针驱动",
                cx,
                |s, _, cx| s.submit(Command::RepairProbe(s.snapshot(cx).get("probe").into()), cx),
            ));
        }
        if let Some(p) = &self.confirmation {
            let request = p.clone();
            page = page.child(
                div()
                    .p_3()
                    .border_1()
                    .border_color(cx.theme().danger)
                    .rounded_md()
                    .child(format!(
                        "确认整片擦除？探针：{}；芯片：{}。全部 Flash 数据将丢失。",
                        p.get("probe"),
                        p.get("chip")
                    ))
                    .child(
                        self.button("confirm-erase", "确认擦除", cx, move |s, _, cx| {
                            s.confirmation = None;
                            s.submit(Command::Erase(request.clone()), cx);
                        }),
                    )
                    .child(self.button("cancel-erase", "取消", cx, |s, _, cx| {
                        s.confirmation = None;
                        cx.notify();
                    })),
            );
        }
        for (index, probe) in self.probes.iter().enumerate() {
            let id = probe.identifier.clone();
            let label = format!("{} · {}", probe.product.as_deref().unwrap_or("探针"), id);
            page = page.child(
                Button::new(("probe", index))
                    .small()
                    .label(label)
                    .on_click(cx.listener(move |s, _, w, cx| s.set("probe", id.clone(), w, cx))),
            );
        }
        for (index, chip) in self.chips.iter().enumerate() {
            let chip = chip.clone();
            page = page.child(
                Button::new(("chip", index))
                    .small()
                    .label(chip.clone())
                    .on_click(cx.listener(move |s, _, w, cx| {
                        s.set("chip", chip.clone(), w, cx);
                        s.chips.clear();
                        cx.notify();
                    })),
            );
        }
        if let Some(p) = &self.preview {
            page = page.child(div().text_sm().child(format!(
                "{:?} · 已用 {} 字节 · 地址跨度 {} 字节",
                p.usage.format, p.usage.used_bytes, p.usage.span_bytes
            )));
            for segment in &p.usage.segments {
                page = page.child(format!(
                    "0x{:08X} — 0x{:08X}    {} 字节",
                    segment.start, segment.end, segment.size
                ));
            }
        }
        for region in &self.layout {
            page = page.child(format!(
                "{:?}  {}  0x{:08X} — 0x{:08X}  {} 字节",
                region.kind,
                region.name.as_deref().unwrap_or(""),
                region.start,
                region.end,
                region.size
            ));
        }
        page.child(
            Textarea::new(&self.firmware_output)
                .font_family(cx.theme().mono_font_family.clone())
                .h(px(220.))
                .readonly(true),
        )
        .into_any_element()
    }
}
