use super::*;
impl Workbench {
    pub(super) fn workflow_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let group = self.form_group(cx);
        let disabled = !self.serial_connected() || self.is_busy();
        let body=match self.serial_page{
            "file"=>group.child(div().flex().items_center().gap_2().child(div().flex_1().child(Input::new(&self.fields["file"]).small().aria_label("文件路径"))).child(self.button("pick-file","选择文件…",cx,|s,_,cx|s.pick("file",cx))))
                .child(self.form_input("chunk","块大小 (字节)",cx)).child(self.form_input("interval","块间隔 (ms)",cx))
                .child(div().py_3().child(self.button("send-file","发送文件",cx,|s,_,cx|s.serial_action("file",cx)).disabled(disabled))),
            "macro"=>group.child(div().text_size(px(11.)).text_color(cx.theme().muted_foreground).child("步骤可包含 send、delayMs、wait 和 timeoutMs。匹配方式为 text、hex 或 regex。"))
                .child(Textarea::new(&self.editors["macro"]).h(px(180.)).font_family(cx.theme().mono_font_family.clone()).text_size(px(13.)))
                .child(self.form_input("cycles","循环次数",cx))
                .child(div().py_3().flex().gap_2().child(self.button("run-macro","运行宏",cx,|s,_,cx|s.serial_action("macro",cx)).disabled(disabled))
                    .child(self.button("macro-report","导出结果 CSV…",cx,|s,_,cx|s.export_workbench("exportMacro","ChipDock-Macro.csv",cx)))),
            "replies"=>group.child(div().text_size(px(11.)).text_color(cx.theme().muted_foreground).child("按顺序匹配，支持 text、hex 或 regex。delayMs 用于延时，enabled 控制是否启用。"))
                .child(Textarea::new(&self.editors["replies"]).h(px(200.)).font_family(cx.theme().mono_font_family.clone()).text_size(px(13.)))
                .child(div().py_3().flex().gap_2().child(self.button("reply-example","示例",cx,|s,w,cx|s.set("replies",r#"[{"name":"pong","enabled":true,"trigger":{"kind":"text","pattern":"PING"},"reply":{"text":"OK","format":"utf-8"},"delayMs":20}]"#,w,cx)))
                    .child(self.button("apply-replies","应用应答规则",cx,|s,_,cx|s.serial_action("replies",cx)))),
            _=>group.child(div().text_size(px(11.)).text_color(cx.theme().muted_foreground).child("每个包可设置 name、group、note、order、enabled、text、format。format 使用 hex 或 utf-8。"))
                .child(Textarea::new(&self.editors["packets"]).h(px(170.)).font_family(cx.theme().mono_font_family.clone()).text_size(px(13.)))
                .child(self.form_input("selected","发送顺序（包名，逗号分隔）",cx)).child(self.form_input("interval","包间隔 (ms)",cx))
                .child(div().py_3().flex().gap_2()
                    .child(self.button("packet-example","示例",cx,|s,w,cx|s.set("packets",r#"[{"name":"读状态","group":"调试","note":"","order":0,"enabled":true,"text":"AA 01","format":"hex"}]"#,w,cx)))
                    .child(self.button("import-packets","导入 JSON…",cx,|s,_,cx|s.import_editor("packets",cx)))
                    .child(self.button("save-packets","保存",cx,|s,w,cx|{s.serial_action("validatePackets",cx);s.save(w,cx);}))
                    .child(self.button("export-packets","导出 JSON…",cx,|s,_,cx|s.export_bytes("ChipDock-Packets.json",s.editors["packets"].read(cx).value().as_bytes().to_vec(),cx)))
                    .child(self.button("batch","批量发送",cx,|s,_,cx|s.serial_action("batch",cx)).disabled(disabled))),
        };
        self.tool_panel(
            &[
                ("packets", "常用包库"),
                ("file", "发送文件"),
                ("macro", "宏命令"),
                ("replies", "自动应答"),
            ],
            body.into_any_element(),
            cx,
        )
    }
    pub(super) fn export_workbench(
        &mut self,
        action: &'static str,
        filename: &str,
        cx: &mut Context<Self>,
    ) {
        let prompt = cx.prompt_for_new_path(std::path::Path::new("."), Some(filename));
        cx.spawn(async move |view, cx| {
            if let Ok(Ok(Some(path))) = prompt.await {
                let _ = view.update_in(cx, |s, w, cx| {
                    s.set("export", path.to_string_lossy().into_owned(), w, cx);
                    s.serial_action(action, cx);
                });
            }
        })
        .detach();
    }
}
