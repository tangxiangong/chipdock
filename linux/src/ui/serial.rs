use super::*;

impl Workbench {
    pub(super) fn serial_page(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let mut content = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(self.terminal_page(window, cx));
        if let Some(status) = self
            .sessions
            .iter()
            .find(|s| Some(s.id) == self.active)
            .and_then(|s| s.status.as_ref())
        {
            if let Some(error) = status
                .last_error
                .as_ref()
                .or(status.job_error.as_ref())
                .or(status.capture_error.as_ref())
                .filter(|e| self.dismissed_serial_error.as_ref() != Some(*e))
            {
                let dismissed = error.clone();
                content = div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .p(px(12.))
                            .flex()
                            .items_start()
                            .gap(px(10.))
                            .bg(rgba(0xff950014))
                            .border_b_1()
                            .border_color(cx.theme().foreground.opacity(0.1))
                            .text_size(px(12.))
                            .child(
                                icons::icon("warning")
                                    .size(px(14.))
                                    .flex_shrink_0()
                                    .text_color(rgb(0xff9500)),
                            )
                            .child(div().flex_1().child(error.clone()))
                            .child(
                                Button::new("dismiss-serial-error")
                                    .ghost()
                                    .size(px(18.))
                                    .p_0()
                                    .tooltip("关闭错误提示")
                                    .child(icons::icon("close").size(px(12.)).flex_shrink_0())
                                    .on_click(cx.listener(move |s, _, _, cx| {
                                        s.dismissed_serial_error = Some(dismissed.clone());
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(content);
            }
            if let Some(feedback) = &status.feedback {
                content = content.child(div().px_3().py_1().text_sm().child(feedback.clone()));
            }
        }
        let mut workspace = h_resizable("serial-workspace");
        if !self.expanded.contains("hide-connection") {
            workspace = workspace.child(
                resizable_panel()
                    .size(px(252.))
                    .size_range(px(220.)..px(340.))
                    .child(self.connection_panel(window, cx)),
            );
        }
        workspace = workspace.child(
            resizable_panel()
                .size_range(px(440.)..px(2400.))
                .child(div().size_full().flex().flex_col().child(content)),
        );
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(32.))
                    .flex_shrink_0()
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .border_b_1()
                    .border_color(cx.theme().foreground.opacity(0.1))
                    .text_size(px(12.))
                    .child(div().flex_1().min_w_0().flex().gap(px(4.)).children(
                        self.sessions.iter().map(|session| {
                            let id = session.id;
                            let selected = Some(id) == self.active;
                            let connected = session.status.as_ref().is_some_and(|s| s.connected);
                            let title = session
                                .status
                                .as_ref()
                                .and_then(|s| s.path.clone())
                                .unwrap_or_else(|| format!("会话 {id}"));
                            div()
                                .flex()
                                .items_center()
                                .child(
                                    Button::new(("session", id as usize))
                                        .ghost()
                                        .h(px(22.))
                                        .px(px(8.))
                                        .rounded(px(6.))
                                        .toggled(selected)
                                        .when(selected, |b| {
                                            b.bg(rgb(if cx.theme().is_dark() {
                                                0x484848
                                            } else {
                                                0xdfdfdf
                                            }))
                                        })
                                        .tooltip(title.clone())
                                        .child(
                                            div()
                                                .max_w(px(200.))
                                                .flex()
                                                .items_center()
                                                .gap(px(5.))
                                                .text_size(px(12.))
                                                .child(div().size(px(6.)).rounded_full().bg(
                                                    if connected {
                                                        rgb(0x34c759).into()
                                                    } else {
                                                        cx.theme().muted_foreground.opacity(0.4)
                                                    },
                                                ))
                                                .child(div().truncate().child(title)),
                                        )
                                        .on_click(cx.listener(move |s, _, w, cx| {
                                            s.switch_session(id, w, cx)
                                        })),
                                )
                                .when(selected && id != 1, |d| {
                                    d.child(
                                        Button::new(("close-session", id as usize))
                                            .ghost()
                                            .size(px(18.))
                                            .p_0()
                                            .mx(px(2.))
                                            .text_color(cx.theme().muted_foreground)
                                            .child(
                                                icons::icon("close").size(px(11.)).flex_shrink_0(),
                                            )
                                            .tooltip("关闭会话")
                                            .disabled(self.busy_lanes.contains(&Lane::Serial))
                                            .on_click(cx.listener(move |s, _, _, cx| {
                                                s.submit(Command::CloseSession(id), cx)
                                            })),
                                    )
                                })
                        }),
                    ))
                    .child(
                        Button::new("local-control")
                            .ghost()
                            .h(px(22.))
                            .px(px(4.))
                            .tooltip("为当前登录用户的 Rust CLI 和 MCP 设置访问权限")
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(5.))
                                    .text_size(px(12.))
                                    .child(icons::icon("terminal").size(px(14.)).flex_shrink_0())
                                    .child("本地控制")
                                    .child(
                                        icons::icon("chevron-bold").size(px(8.)).flex_shrink_0(),
                                    ),
                            )
                            .dropdown_menu({
                                let owner = cx.entity().downgrade();
                                let current = self.preferences.ipc;
                                move |mut menu, _, _| {
                                    for (mode, label) in [(0, "关闭"), (1, "只读"), (2, "允许控制")]
                                    {
                                        let owner = owner.clone();
                                        menu = menu.item(
                                            PopupMenuItem::new(label)
                                                .checked(current == mode)
                                                .on_click(move |_, _, cx| {
                                                    let _ = owner.update(cx, |s, cx| {
                                                        s.submit(Command::Ipc(mode), cx)
                                                    });
                                                }),
                                        );
                                    }
                                    menu
                                }
                            }),
                    )
                    .child(
                        div()
                            .w(px(1.))
                            .h(px(16.))
                            .bg(cx.theme().foreground.opacity(0.12)),
                    )
                    .child(
                        Button::new("new-session")
                            .ghost()
                            .size(px(22.))
                            .p_0()
                            .text_color(cx.theme().foreground.opacity(0.7))
                            .child(icons::icon("plus").size(px(14.)).flex_shrink_0())
                            .tooltip("新建串口会话")
                            .disabled(
                                self.busy_lanes.contains(&Lane::Serial)
                                    || self.sessions.len() >= 16,
                            )
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.submit(Command::NewSession(s.snapshot(cx)), cx)
                            })),
                    ),
            )
            .child(div().flex_1().min_h_0().child(workspace))
            .child(self.serial_status_bar(cx))
            .into_any_element()
    }

    pub(super) fn open_serial_tool(
        &mut self,
        key: &'static str,
        title: &'static str,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if key == "protocol" {
            self.open_protocol(w, cx);
            return;
        }
        self.serial_page = if key == "tools" { "checksum" } else { key };
        self.show_text(String::new(), w, cx);
        let owner = cx.entity().downgrade();
        let closing = owner.clone();
        if !w.has_active_dialog(cx) {
            self.focus_handle.focus(w, cx);
        }
        w.open_dialog(cx, move |dialog, w, cx| {
            let body = owner
                .update(cx, |s, cx| s.serial_tool_body(w, cx))
                .unwrap_or_else(|_| div().into_any_element());
            let closing = closing.clone();
            let tools = matches!(key, "tools" | "checksum" | "framing" | "modbus" | "capture");
            let workflow = matches!(key, "packets" | "file" | "macro" | "replies");
            let records = key == "records";
            dialog
                .when(!tools && !workflow && !records, |d| d.title(title))
                .p_0()
                .centered()
                .close_button(!tools && !workflow && !records)
                .w(px(if tools {
                    540.
                } else if workflow {
                    660.
                } else if records {
                    920.
                } else {
                    760.
                }))
                .child(
                    div()
                        .id("serial-dialog-body")
                        .text_size(px(13.))
                        .line_height(relative(1.2))
                        .max_h(px(640.))
                        .overflow_y_scroll()
                        .child(body),
                )
                .on_close(move |_, _, cx| {
                    let _ = closing.update(cx, |s, cx| {
                        s.serial_page = "terminal";
                        cx.notify();
                    });
                })
        });
    }
    fn serial_tool_body(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if matches!(
            self.serial_page,
            "checksum" | "framing" | "modbus" | "capture"
        ) {
            return self.tools_page(cx);
        }
        if matches!(self.serial_page, "packets" | "file" | "macro" | "replies") {
            return self.workflow_page(cx);
        }
        if self.serial_page == "records" {
            return self.records_page(cx).into_any_element();
        }
        let body = match self.serial_page {
            "terminal" => div().size_full().child(self.terminal_page(window,cx)),
            "records" => self.records_page(cx),
            "packets" => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(self.editor("packets", cx))
                .child(self.row(&["selected", "interval"]))
                .child(self.button("validate-packets", "验证包库", cx, |s, _, cx| {
                    s.serial_action("validatePackets", cx)
                }))
                .child(self.button("batch", "批量发送", cx, |s, _, cx| {
                    s.serial_action("batch", cx)
                })),
            "file" => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(self.row(&["file", "chunk", "interval"]))
                .child(self.button("pick-file", "选择文件…", cx, |s, _, cx| {
                    s.pick("file", cx)
                }))
                .child(self.button("send-file", "发送文件", cx, |s, _, cx| {
                    s.serial_action("file", cx)
                })),
            "macro" => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(self.editor("macro", cx))
                .child(self.field("cycles"))
                .child(self.button("run-macro", "执行宏", cx, |s, _, cx| {
                    s.serial_action("macro", cx)
                }))
                .child(self.field("export"))
                .child(self.button("macro-report", "导出宏报告", cx, |s, _, cx| {
                    s.serial_action("exportMacro", cx)
                })),
            "replies" => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(self.editor("replies", cx))
                .child(
                    self.button("apply-replies", "应用自动应答", cx, |s, _, cx| {
                        s.serial_action("replies", cx)
                    }),
                ),
            "protocol" => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(self.editor("protocol", cx))
                .child(
                    self.button("apply-protocol", "应用协议模板", cx, |s, _, cx| {
                        s.serial_action("protocol", cx)
                    }),
                )
                .child(self.field("protocol_sample"))
                .child(self.button("decode-sample","测试接收解析",cx,|s,_,cx|{let p=s.snapshot(cx);match serde_json::from_str::<Value>(p.get("protocol")){Ok(rules)=>s.submit(Command::Protocol(json!({"operation":"decode","rules":rules,"incoming":p.get("protocol_sample")})),cx),Err(e)=>s.report(e.to_string(),cx)}}))
                 .child(self.field("protocol_rule"))
                .child(self.button("inspect-protocol","查看协议帧布局",cx,|s,_,cx|{let p=s.snapshot(cx);match serde_json::from_str::<Value>(p.get("protocol")){Ok(rules)=>s.submit(Command::Protocol(json!({"operation":"inspect","rules":rules,"input":p.get("command"),"incoming":p.get("protocol_sample"),"rule":p.get("protocol_rule")})),cx),Err(e)=>s.report_error(e.to_string(),cx)}}))
                .child(self.field("command"))
                .child(div().flex().flex_wrap().gap_1().children(self.preferences.command_history.iter().take(12).enumerate().map(|(index,text)|{let text=text.clone();native_button(("command-history",index)).label(text.clone()).on_click(cx.listener(move |s,_,w,cx|s.set("command",text.clone(),w,cx)))})))
                .child(self.button("preview-command","预览命令编码",cx,|s,_,cx|{let p=s.snapshot(cx);match serde_json::from_str::<Value>(p.get("protocol")){Ok(rules)=>s.submit(Command::Protocol(json!({"operation":"encode","rules":rules,"input":p.get("command")})),cx),Err(e)=>s.report(e.to_string(),cx)}}))
                .child(self.button("command", "发送协议命令", cx, |s, _, cx| {
                    s.serial_action("command", cx)
                })),
            "capture" => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(self.row(&["capture_path", "roll"]))
                .child(self.button("capture-directory","选择采集目录…",cx,|s,_,cx|s.pick_capture_directory(cx)))
                .child(self.choice("capture_format", &["txt", "csv", "bin"], window, cx))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(self.button("capture", "开始日志", cx, |s, _, cx| {
                            s.serial_action("capture", cx)
                        }))
                        .child(self.button("stop-capture", "停止日志", cx, |s, _, cx| {
                            s.serial_action("stopCapture", cx)
                        }))
                        .child(self.button("raw", "开始原始采集", cx, |s, _, cx| {
                            s.serial_action("raw", cx)
                        }))
                        .child(self.button("stop-raw", "停止原始采集", cx, |s, _, cx| {
                            s.serial_action("stopRaw", cx)
                        })),
                )
                .child(self.field("export"))
                .child(self.button("serial-export", "导出记录", cx, |s, _, cx| {
                    s.serial_action("export", cx)
                })),
            "plot" => self.plot_page(cx),
            _ => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(self.button("calculate", "计算发送校验", cx, |s, _, cx| {
                    s.serial_action("checksum", cx)
                }))
                .child(self.choice(
                    "frame_mode",
                    &["read", "idle", "header", "tail", "fixed"],
                    window,
                    cx,
                ))
                .child(self.field("frame_value"))
                .child(self.button("framing", "应用分帧", cx, |s, _, cx| {
                    s.serial_action("framing", cx)
                }))
                .child(div().text_lg().child("Modbus RTU"))
                .child(self.row(&["station", "register", "quantity", "timeout"]))
                .child(self.choice(
                    "function",
                    &["1", "2", "3", "4", "5", "6", "15", "16"],
                    window,
                    cx,
                ))
                .child(self.field("values"))
                .child(self.button("modbus", "生成请求", cx, |s, _, cx| {
                    s.serial_action("modbus", cx)
                }))
                .child(
                    self.button("modbus-send", "发送并接收响应", cx, |s, _, cx| {
                        s.serial_action("modbusSend", cx)
                    }),
                ),
        };

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(body)
            .child(self.button("cancel-job", "停止发送任务", cx, |s, _, cx| {
                s.serial_action("cancelJob", cx)
            }))
            .child(Textarea::new(&self.output).h(px(120.)).readonly(true))
            .into_any_element()
    }
}
