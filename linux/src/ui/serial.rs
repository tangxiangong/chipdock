use super::*;

impl Workbench {
    pub(super) fn serial_page(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let mut page = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .gap_2()
                    .children(self.sessions.iter().map(|session| {
                        let id = session.id;
                        Button::new(("session", id as usize))
                            .small()
                            .label(format!("串口 {id}"))
                            .selected(Some(id) == self.active)
                            .on_click(cx.listener(move |s, _, w, cx| s.switch_session(id, w, cx)))
                    }))
                    .child(self.button("new-session", "新建会话", cx, |s, _, cx| {
                        s.submit(Command::NewSession(s.snapshot(cx)), cx)
                    }))
                    .child(self.button("close-session", "关闭会话", cx, |s, _, cx| {
                        if s.sessions.len() > 1
                            && let Some(id) = s.active
                        {
                            s.submit(Command::CloseSession(id), cx)
                        }
                    })),
            )
            .child(self.row(&["port", "baud", "bits", "stop"]))
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(self.choice(
                        "parity",
                        &["none", "odd", "even", "mark", "space"],
                        window,
                        cx,
                    ))
                    .child(self.choice("flow", &["none", "software", "hardware"], window, cx)),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.button("ports", "刷新串口", cx, |s, _, cx| {
                        s.submit(Command::Ports, cx)
                    }))
                    .child(self.button("serial-connect", "连接", cx, |s, _, cx| {
                        if let Some(id) = s.active {
                            s.submit(Command::ConnectSerial(id, s.snapshot(cx)), cx)
                        } else {
                            s.report("请先新建会话", cx)
                        }
                    }))
                    .child(self.button("disconnect", "断开", cx, |s, _, cx| {
                        if let Some(id) = s.active {
                            s.submit(Command::Disconnect(id), cx)
                        }
                    }))
                    .child(self.flag("reconnect", "自动重连", cx))
                    .child(self.flag("auto_connect", "启动时自动连接", cx)),
            );
        for (index, port) in self.ports.iter().enumerate() {
            let path = port.path.clone();
            page = page.child(
                Button::new(("port", index))
                    .small()
                    .label(format!("{} · {}", port.display_name, path))
                    .on_click(cx.listener(move |s, _, w, cx| s.set("port", path.clone(), w, cx))),
            );
        }
        if let Some(status) = self
            .sessions
            .iter()
            .find(|s| Some(s.id) == self.active)
            .and_then(|s| s.status.as_ref())
        {
            page = page.child(format!(
                "{} · RX {} B ({} B/s) · TX {} B ({} B/s) · {} / {}",
                if status.connected {
                    "已连接"
                } else if status.reconnecting {
                    "重连中"
                } else {
                    "未连接"
                },
                status.rx_bytes,
                status.rx_per_second,
                status.tx_bytes,
                status.tx_per_second,
                status.job_done,
                status.job_total
            ));
            for (line, label, value) in [
                (ControlLine::Rts, "RTS", status.rts),
                (ControlLine::Dtr, "DTR", status.dtr),
            ] {
                page = page.child(
                    Button::new(label)
                        .small()
                        .label(format!(
                            "{label}: {}",
                            value.map_or("未知", |v| if v { "高" } else { "低" })
                        ))
                        .on_click(cx.listener(move |s, _, _, cx| {
                            if let Some(id) = s.active {
                                s.submit(Command::Line(id, line, !value.unwrap_or(false)), cx)
                            }
                        })),
                );
            }
            if let Some(error) = status
                .last_error
                .as_ref()
                .or(status.job_error.as_ref())
                .or(status.capture_error.as_ref())
            {
                page = page.child(div().text_color(cx.theme().danger).child(error.clone()));
            }
        }
        if let Some(feedback) = self
            .sessions
            .iter()
            .find(|s| Some(s.id) == self.active)
            .and_then(|s| s.status.as_ref())
            .and_then(|s| s.feedback.clone())
        {
            page = page.child(feedback);
        }
        page = page.child(
            div().flex().flex_wrap().gap_1().children(
                [
                    ("terminal", "终端"),
                    ("records", "记录"),
                    ("packets", "包库"),
                    ("file", "发送文件"),
                    ("macro", "宏命令"),
                    ("replies", "自动应答"),
                    ("protocol", "协议"),
                    ("tools", "工具"),
                    ("capture", "采集"),
                    ("plot", "曲线"),
                ]
                .into_iter()
                .map(|(key, label)| {
                    Button::new(key)
                        .small()
                        .label(label)
                        .selected(self.serial_page == key)
                        .on_click(cx.listener(move |s, _, _, cx| {
                            s.serial_page = key;
                            cx.notify();
                        }))
                }),
            ),
        );
        let body = match self.serial_page {
            "terminal" => self.terminal_page(window,cx),
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
                .child(div().flex().flex_wrap().gap_1().children(self.preferences.command_history.iter().take(12).enumerate().map(|(index,text)|{let text=text.clone();Button::new(("command-history",index)).label(text.clone()).on_click(cx.listener(move |s,_,w,cx|s.set("command",text.clone(),w,cx)))})))
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
        page.child(body)
            .child(self.button("cancel-job", "停止发送任务", cx, |s, _, cx| {
                s.serial_action("cancelJob", cx)
            }))
            .child(Textarea::new(&self.output).h(px(140.)).readonly(true))
            .into_any_element()
    }
}
