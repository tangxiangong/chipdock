use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Panel {
    Memory,
    Logs,
    Settings,
    Plot(u64),
}
struct Auxiliary {
    focus_handle: FocusHandle,
    owner: Entity<Workbench>,
    panel: Panel,
    plot_mode: String,
    names: Entity<InputState>,
    template: Entity<InputState>,
    binary: Entity<TextareaState>,
    sample: Entity<InputState>,
    filter: Entity<InputState>,
    message: String,
    busy: bool,
}
impl Workbench {
    pub(super) fn open_panel(&mut self, panel: Panel, cx: &mut Context<Self>) -> Result<()> {
        if panel == Panel::Logs {
            self.unseen_errors = 0;
            cx.notify();
        }
        if let Some((_, handle)) = self.auxiliary.iter().find(|(kind, _)| *kind == panel)
            && (handle.update(cx, |_, w, _| w.activate_window()).is_ok()
                || cx
                    .windows()
                    .iter()
                    .any(|window| window.window_id() == handle.window_id()))
        {
            return Ok(());
        }
        if let Panel::Plot(id) = panel {
            if Some(id) == self.active {
                self.plotting = true;
            } else if let Some(session) = self.sessions.iter_mut().find(|s| s.id == id) {
                session.plotting = true;
            }
        }
        let owner = cx.entity();
        let config = match panel {
            Panel::Plot(id) if Some(id) == self.active => self.snapshot(cx).get("plot").to_owned(),
            Panel::Plot(id) => self
                .sessions
                .iter()
                .find(|s| s.id == id)
                .map(|s| s.preferences.get("plot").to_owned())
                .context("会话已关闭")?,
            _ => "{}".into(),
        };
        let title: String = match panel {
            Panel::Memory => "目标内存".into(),
            Panel::Logs => "活动日志".into(),
            Panel::Settings => "设置".into(),
            Panel::Plot(_) => "实时曲线".into(),
        };
        let (width, height) = match panel {
            Panel::Memory => (560., 600.),
            Panel::Logs => (720., 440.),
            Panel::Settings => (480., 420.),
            Panel::Plot(_) => (800., 500.),
        };
        let custom_title = matches!(panel, Panel::Settings | Panel::Logs);
        // Opening a window draws it immediately. Release the Workbench update
        // before Auxiliary::render reads its owner.
        cx.defer(move |cx| {
            let panel_owner = owner.clone();
            let result = gpui_kit::open_window(
                WindowOptions {
                    titlebar: Some(TitlebarOptions {
                        title: Some(title.into()),
                        ..if custom_title {
                            gpui_kit::component::TitleBar::title_bar_options()
                        } else {
                            TitlebarOptions::default()
                        }
                    }),
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(width), px(height)),
                        cx,
                    ))),
                    window_min_size: Some(size(px(width.min(640.)), px(height.min(420.)))),
                    app_owns_titlebar_drag: custom_title,
                    ..Default::default()
                },
                cx,
                |w, cx| {
                    cx.new(|cx| {
                        cx.observe(&panel_owner, |_, _, cx| cx.notify()).detach();
                        let filter = cx.new(|cx| InputState::new(w, cx));
                        cx.observe(&filter, |_, _, cx| cx.notify()).detach();
                        let focus_handle = cx.focus_handle();
                        focus_handle.focus(w, cx);
                        let parsed: Value = serde_json::from_str(&config).unwrap_or_default();
                        let names = parsed["fields"]
                            .as_array()
                            .map(|v| {
                                v.iter()
                                    .filter_map(Value::as_str)
                                    .collect::<Vec<_>>()
                                    .join(",")
                            })
                            .unwrap_or_default();
                        let sample = cx.new(|cx| InputState::new(w, cx).placeholder("样例文本"));
                        cx.observe(&sample, |_, _, cx| cx.notify()).detach();
                        Auxiliary {
                            focus_handle,
                            owner: panel_owner,
                            panel,
                            plot_mode: parsed["mode"].as_str().unwrap_or("numbers").into(),
                            names: cx.new(|cx| {
                                InputState::new(w, cx)
                                    .placeholder("字段名，逗号分隔")
                                    .default_value(names)
                            }),
                            template: cx.new(|cx| {
                                InputState::new(w, cx)
                                    .placeholder("协议规则名，可选")
                                    .default_value(
                                        parsed["template"].as_str().unwrap_or("").to_owned(),
                                    )
                            }),
                            binary: cx.new(|cx| {
                                TextareaState::new(w, cx).default_value(
                                    parsed
                                        .get("binary")
                                        .map(Value::to_string)
                                        .unwrap_or_else(|| "[]".into()),
                                )
                            }),
                            sample,
                            filter,
                            message: String::new(),
                            busy: false,
                        }
                    })
                },
            );
            owner.update(cx, |owner, cx| match result {
                Ok((handle, _)) => {
                    owner.auxiliary.retain(|(kind, _)| *kind != panel);
                    owner.auxiliary.push((panel, handle));
                }
                Err(error) => owner.report_error(error.to_string(), cx),
            });
        });
        Ok(())
    }
    pub(super) fn close_panels(&mut self, cx: &mut Context<Self>) {
        for (_, handle) in std::mem::take(&mut self.auxiliary) {
            let _ = handle.update(cx, |_, w, _| w.remove_window());
        }
    }
}
impl Auxiliary {
    fn with_main(
        &self,
        operation: impl FnOnce(&mut Workbench, &mut Window, &mut Context<Workbench>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let owner = self.owner.clone();
        let main = owner.read(cx).main_window;
        cx.defer(move |cx| {
            let _ = main.update(cx, |_, window, cx| {
                owner.update(cx, |owner, cx| {
                    operation(owner, window, cx);
                });
            });
        });
    }

    fn configure(&mut self, preview: bool, w: &mut Window, cx: &mut Context<Self>) {
        let Panel::Plot(id) = self.panel else { return };
        let mut config =
            json!({"mode":self.plot_mode,"template":self.template.read(cx).value().to_string()});
        if self.plot_mode == "binary" {
            match serde_json::from_str::<Value>(self.binary.read(cx).value().as_ref()) {
                Ok(value) => config["binary"] = value,
                Err(e) => {
                    self.message = e.to_string();
                    cx.notify();
                    return;
                }
            }
        } else {
            config["fields"] = json!(
                self.names
                    .read(cx)
                    .value()
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
            );
        }
        let source = config.to_string();
        let sample = if preview {
            self.sample.read(cx).value().to_string()
        } else {
            String::new()
        };
        let hex = config["mode"] == "binary";
        let task = cx.background_executor().spawn(async move {
            service::serial_workbench(
                id,
                &json!({"action":"plot","config":config,"sample":sample,"hex":hex}).to_string(),
            )
        });
        self.busy = true;
        cx.spawn_in(w, async move |view, cx| {
            let result = task.await;
            let _ = view.update_in(cx, |s, w, cx| {
                s.busy = false;
                match result {
                    Err(e) => s.message = e.to_string(),
                    Ok(value) => {
                        s.message = if preview { value } else { String::new() };
                        if !preview {
                            s.owner.update(cx, |owner, cx| {
                                if Some(id) == owner.active {
                                    owner.editors["plot"]
                                        .update(cx, |e, cx| e.set_value(source.clone(), w, cx));
                                    owner.plot_config = source.clone();
                                    owner.plot_revision += 1;
                                    owner.plot_points.clear();
                                    owner.plot_dropped = 0;
                                } else if let Some(session) =
                                    owner.sessions.iter_mut().find(|s| s.id == id)
                                {
                                    session
                                        .preferences
                                        .fields
                                        .insert("plot".into(), source.clone());
                                    session.plot_config = source;
                                    session.plot_revision += 1;
                                    session.plot_points.clear();
                                    session.plot_dropped = 0;
                                }
                                cx.notify();
                            });
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn plot(&mut self, id: u64, cx: &mut Context<Self>) -> AnyElement {
        let owner = self.owner.read(cx);
        let Some(session) = owner.sessions.iter().find(|s| s.id == id) else {
            return div().child("串口会话已关闭").into_any_element();
        };
        let (points, paused, dropped) = if Some(id) == owner.active {
            (
                owner.plot_points.clone(),
                !owner.plotting,
                owner.plot_dropped,
            )
        } else {
            (
                session.plot_points.clone(),
                !session.plotting,
                session.plot_dropped,
            )
        };
        let field = self.filter.read(cx).value().to_string();
        let bytes = chipdock_gpui::presentation::plot_csv(&points, &field);
        let empty = points.is_empty();
        let mut series: BTreeMap<String, Vec<(f64, f64)>> = BTreeMap::new();
        let mut fields = std::collections::BTreeSet::new();
        for point in &points {
            if let Some(values) = point["values"].as_object() {
                for (name, value) in values {
                    fields.insert(name.clone());
                    if (field.is_empty() || field == *name)
                        && let Some(y) = value.as_f64()
                    {
                        series
                            .entry(name.clone())
                            .or_default()
                            .push((point["seq"].as_f64().unwrap_or(0.), y));
                    }
                }
            }
        }
        let mut page = div()
            .size_full()
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("会话 {id}")),
                    )
                    .child(
                        native_button("plot-pause")
                            .small()
                            .icon(icons::icon("pause"))
                            .label(if paused { "继续" } else { "暂停" })
                            .on_click(cx.listener(move |s, _, _, cx| {
                                s.owner.update(cx, |o, cx| {
                                    if Some(id) == o.active {
                                        o.plotting = !o.plotting;
                                    } else if let Some(session) =
                                        o.sessions.iter_mut().find(|s| s.id == id)
                                    {
                                        session.plotting = !session.plotting;
                                    }
                                    cx.notify();
                                })
                            })),
                    )
                    .child(
                        native_button("plot-clear")
                            .small()
                            .icon(icons::icon("trash"))
                            .label("清空")
                            .on_click(cx.listener(move |s, _, _, cx| {
                                s.owner.update(cx, |o, cx| {
                                    if Some(id) == o.active {
                                        o.plot_revision += 1;
                                        o.plot_points.clear();
                                        o.plot_dropped = 0;
                                    } else if let Some(session) =
                                        o.sessions.iter_mut().find(|s| s.id == id)
                                    {
                                        session.plot_revision += 1;
                                        session.plot_points.clear();
                                        session.plot_dropped = 0;
                                    }
                                    cx.notify();
                                })
                            })),
                    )
                    .child(
                        native_button("plot-export")
                            .small()
                            .icon(icons::icon("export"))
                            .label("导出 CSV…")
                            .disabled(empty)
                            .on_click(cx.listener(move |s, _, _, cx| {
                                s.owner.update(cx, |o, cx| {
                                    o.export_bytes("ChipDock-Plot.csv", bytes.clone(), cx)
                                })
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child("输入")
                    .child(
                        native_button("plot-mode")
                            .small()
                            .w(px(135.))
                            .label(match self.plot_mode.as_str() {
                                "kv" => "键值对",
                                "json" => "JSON",
                                "binary" => "二进制",
                                _ => "数值 / 分隔值",
                            })
                            .child(icons::icon("select"))
                            .dropdown_menu({
                                let owner = cx.entity().downgrade();
                                move |mut m, _, _| {
                                    for (mode, label) in [
                                        ("numbers", "数值 / 分隔值"),
                                        ("kv", "键值对"),
                                        ("json", "JSON"),
                                        ("binary", "二进制"),
                                    ] {
                                        let owner = owner.clone();
                                        m = m.item(PopupMenuItem::new(label).on_click(
                                            move |_, _, cx| {
                                                let _ = owner.update(cx, |s, cx| {
                                                    s.plot_mode = mode.into();
                                                    cx.notify();
                                                });
                                            },
                                        ));
                                    }
                                    m
                                }
                            }),
                    )
                    .child(
                        div().flex_1().child(
                            Input::new(&self.names)
                                .small()
                                .disabled(self.plot_mode == "binary"),
                        ),
                    )
                    .child(div().flex_1().child(Input::new(&self.template).small()))
                    .child(
                        native_button("plot-apply")
                            .small()
                            .label("应用")
                            .disabled(self.busy)
                            .on_click(cx.listener(|s, _, w, cx| s.configure(false, w, cx))),
                    ),
            )
            .when(self.plot_mode == "binary", |v| {
                v.child(
                    Textarea::new(&self.binary)
                        .h(px(70.))
                        .aria_label("字段配置 JSON"),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().flex_1().child(Input::new(&self.sample).small()))
                    .child(
                        native_button("plot-preview")
                            .small()
                            .label("试算")
                            .disabled(self.busy || self.sample.read(cx).value().is_empty())
                            .on_click(cx.listener(|s, _, w, cx| s.configure(true, w, cx))),
                    )
                    .child("显示字段")
                    .child(
                        native_button("plot-fields")
                            .small()
                            .w(px(90.))
                            .label(if field.is_empty() {
                                "全部".into()
                            } else {
                                field.clone()
                            })
                            .child(icons::icon("select"))
                            .dropdown_menu({
                                let owner = cx.entity().downgrade();
                                move |mut m, _, _| {
                                    for name in
                                        std::iter::once(String::new()).chain(fields.iter().cloned())
                                    {
                                        let owner = owner.clone();
                                        m = m.item(
                                            PopupMenuItem::new(if name.is_empty() {
                                                "全部".into()
                                            } else {
                                                name.clone()
                                            })
                                            .on_click(move |_, w, cx| {
                                                let _ = owner.update(cx, |s, cx| {
                                                    s.filter.update(cx, |f, cx| {
                                                        f.set_value(name.clone(), w, cx)
                                                    });
                                                    cx.notify();
                                                });
                                            }),
                                        );
                                    }
                                    m
                                }
                            }),
                    ),
            );
        if !self.message.is_empty() {
            page = page.child(
                div()
                    .text_size(px(11.))
                    .text_color(cx.theme().muted_foreground)
                    .child(self.message.clone()),
            );
        }
        if dropped > 0 {
            page = page.child(format!("已超出保留范围 {dropped} 条记录"));
        }
        if empty {
            page = page
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap_3()
                        .text_color(cx.theme().muted_foreground)
                        .child(
                            icons::icon("plot")
                                .size(px(38.))
                                .text_color(cx.theme().muted_foreground.opacity(0.5)),
                        )
                        .child(
                            div()
                                .text_size(px(15.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("暂无曲线数据"),
                        )
                        .child(if paused {
                            "已暂停采样。"
                        } else {
                            "收到可解析的数值后会实时绘制。"
                        }),
                )
                .child(
                    div()
                        .h(px(20.))
                        .text_size(px(11.))
                        .text_color(cx.theme().muted_foreground)
                        .child("样本"),
                );
        } else {
            page = page.child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(super::plot_surface::surface(series, cx)),
            );
        }
        page.into_any_element()
    }
}

impl Render for Auxiliary {
    fn render(&mut self, w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match self.panel {
            Panel::Plot(id) => self.plot(id, cx),
            panel => self.owner.update(cx, |o, cx| {
                let previous = o.page;
                o.page = match panel {
                    Panel::Memory => "memory",
                    Panel::Settings => "settings",
                    _ => "logs",
                };
                let view = match panel {
                    Panel::Memory => o.memory_page(cx),
                    Panel::Settings => o.settings_page(cx),
                    _ => o.logs_page(cx),
                };
                o.page = previous;
                view
            }),
        };
        let _ = w;
        let content = if matches!(
            self.panel,
            Panel::Memory | Panel::Logs | Panel::Settings | Panel::Plot(_)
        ) {
            div().size_full().child(body).into_any_element()
        } else {
            div()
                .id("panel-scroll")
                .size_full()
                .overflow_y_scroll()
                .p_4()
                .child(body)
                .into_any_element()
        };
        div()
            .size_full()
            .track_focus(&self.focus_handle)
            .text_size(px(13.))
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(|s, _: &ShowSettings, _, cx| {
                s.with_main(
                    |o, _, cx| {
                        if let Err(e) = o.open_panel(Panel::Settings, cx) {
                            o.report_error(e.to_string(), cx);
                        }
                    },
                    cx,
                )
            }))
            .on_action(cx.listener(|s, _: &ShowLogs, _, cx| {
                s.with_main(
                    |o, _, cx| {
                        if let Err(e) = o.open_panel(Panel::Logs, cx) {
                            o.report_error(e.to_string(), cx);
                        }
                    },
                    cx,
                )
            }))
            .on_action(cx.listener(|s, _: &ShowMemory, _, cx| {
                s.with_main(
                    |o, _, cx| {
                        if let Err(e) = o.open_panel(Panel::Memory, cx) {
                            o.report_error(e.to_string(), cx);
                        }
                    },
                    cx,
                )
            }))
            .on_action(cx.listener(|s, _: &ShowFlash, _, cx| {
                s.with_main(
                    |o, w, cx| {
                        w.activate_window();
                        o.focus_handle.focus(w, cx);
                        o.page = "flash";
                        cx.notify();
                    },
                    cx,
                )
            }))
            .on_action(cx.listener(|s, _: &ShowSerial, _, cx| {
                s.with_main(
                    |o, w, cx| {
                        w.activate_window();
                        o.focus_handle.focus(w, cx);
                        o.page = "serial";
                        cx.notify();
                    },
                    cx,
                )
            }))
            .on_action(cx.listener(|s, _: &OpenFirmware, _, cx| {
                s.with_main(|o, _, cx| o.pick("firmware", cx), cx)
            }))
            .on_action(cx.listener(|s, _: &Find, w, cx| {
                if s.panel == Panel::Logs {
                    s.owner.update(cx, |o, cx| {
                        o.fields["log_search"].update(cx, |field, cx| field.focus(w, cx))
                    });
                }
            }))
            .on_action(cx.listener(|s, _: &ReadMemory, w, cx| {
                if s.panel == Panel::Memory && !w.has_active_dialog(cx) {
                    s.owner.update(cx, |o, cx| {
                        if o.target_connected(cx) && !o.is_busy() && o.confirmation.is_none() {
                            o.submit(Command::Memory(o.snapshot(cx)), cx);
                        }
                    });
                }
            }))
            .on_action(cx.listener(|_, _: &CloseWindow, w, cx| {
                if w.has_active_dialog(cx) {
                    w.close_dialog(cx);
                } else {
                    w.remove_window();
                }
            }))
            .on_action(cx.listener(|s, _: &Quit, w, cx| {
                s.owner.update(cx, |o, cx| {
                    if o.busy_lanes.iter().all(|l| l.background())
                        && o.sessions.iter().all(|s| {
                            s.status
                                .as_ref()
                                .is_none_or(|s| s.job_kind.is_none() && !s.logging && !s.capturing)
                        })
                    {
                        o.save(w, cx);
                        cx.quit();
                    } else {
                        o.report_error("请先停止任务和采集", cx);
                    }
                })
            }))
            .child(content)
    }
}
