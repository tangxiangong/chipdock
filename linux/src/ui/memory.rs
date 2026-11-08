use super::*;

impl Workbench {
    pub(super) fn memory_page(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(div().text_xl().child("目标内存"))
            .child(
                self.button("detach-memory", "在独立窗口打开", cx, |s, _, cx| {
                    if let Err(e) = s.open_panel(super::auxiliary::Panel::Memory, cx) {
                        s.report_error(e.to_string(), cx);
                    }
                }),
            )
            .child(self.button(
                "memory-layout",
                "查看目标内存映射",
                cx,
                |s, _, cx| s.submit(Command::Layout(s.snapshot(cx)), cx),
            ))
            .children(self.layout.iter().enumerate().map(|(index, region)| {
                let address = region.start;
                let length = region.size.min(4096);
                Button::new(("memory-region", index))
                    .label(format!(
                        "{:?} · 0x{:X}—0x{:X} · {} 字节",
                        region.kind, region.start, region.end, region.size
                    ))
                    .on_click(cx.listener(move |s, _, w, cx| {
                        s.set("address", format!("0x{address:X}"), w, cx);
                        s.set("length", length.to_string(), w, cx);
                    }))
            }))
            .child(
                self.button("memory-erase", "擦除整个芯片…", cx, |s, _, cx| {
                    s.confirmation = Some(s.snapshot(cx));
                    s.page = "flash";
                    let _ = s.main_window.update(cx, |_, w, _| w.activate_window());
                    cx.notify();
                }),
            )
            .child(self.row(&["address", "length"]))
            .child(self.button("read", "读取内存", cx, |s, _, cx| {
                s.submit(Command::Memory(s.snapshot(cx)), cx)
            }))
            .child(
                Textarea::new(&self.memory_output)
                    .font_family(cx.theme().mono_font_family.clone())
                    .h(px(360.))
                    .readonly(true),
            )
            .child(self.field("export"))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.button("memory-bin", "导出 BIN", cx, |s, _, cx| {
                        if let Some(m) = &s.memory {
                            match crate::model::decode_hex(&m.data_hex) {
                                Ok(bytes) => s.export_bytes("ChipDock-memory.bin", bytes, cx),
                                Err(e) => s.report_error(e.to_string(), cx),
                            }
                        }
                    }))
                    .child(self.button("memory-copy", "复制数据", cx, |s, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(
                            s.memory_output.read(cx).value().to_string(),
                        ))
                    })),
            )
            .into_any_element()
    }
}
