use super::*;

impl Workbench {
    pub(super) fn profiles_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut page = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_xl().child("配置与最近文件"))
            .child(self.field("profile"))
            .child(self.button("save-profile", "存储当前配置", cx, |s, w, cx| {
                if let Err(error) = s.store_profile(w, cx) {
                    s.report_error(error.to_string(), cx);
                }
            }));
        for (index, (name, p)) in self.preferences.profiles.iter().enumerate() {
            let p = p.clone();
            let name = name.clone();
            let remove = name.clone();
            page = page.child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        native_button(("profile", index))
                            .label(name)
                            .on_click(cx.listener(move |s, _, w, cx| {
                                s.apply_profile(&p, w, cx);
                            })),
                    )
                    .child(
                        native_button(("remove-profile", index))
                            .label("删除")
                            .on_click(cx.listener(move |s, _, w, cx| {
                                s.preferences.profiles.remove(&remove);
                                s.save(w, cx);
                            })),
                    ),
            );
        }
        page = page.child(
            div()
                .flex()
                .gap_2()
                .child(self.button("prune-recent", "清除失效记录", cx, |s, w, cx| {
                    s.preferences
                        .recent
                        .retain(|p| std::path::Path::new(p).is_file());
                    s.save(w, cx);
                }))
                .child(self.button("clear-recent", "清空最近文件", cx, |s, w, cx| {
                    s.preferences.recent.clear();
                    s.save(w, cx);
                })),
        );
        for (index, path) in self.preferences.recent.iter().enumerate() {
            let path = path.clone();
            let reveal = path.clone();
            let remove = path.clone();
            page = page.child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        native_button(("reveal-recent", index))
                            .label("显示文件")
                            .on_click(move |_, _, cx| {
                                cx.reveal_path(std::path::Path::new(&reveal))
                            }),
                    )
                    .child(
                        native_button(("remove-recent", index))
                            .label("从历史移除")
                            .on_click(cx.listener(move |s, _, w, cx| {
                                s.preferences.recent.retain(|p| p != &remove);
                                s.save(w, cx);
                            })),
                    ),
            );
            page = page.child(
                native_button(("recent", index))
                    .label(path.clone())
                    .on_click(cx.listener(move |s, _, w, cx| {
                        s.set("firmware", path.clone(), w, cx);
                        let base = s
                            .preferences
                            .recent_bases
                            .get(&path)
                            .cloned()
                            .unwrap_or_default();
                        s.set("base", base, w, cx);
                        s.page = "flash";
                        s.submit(Command::Preview(s.snapshot(cx)), cx);
                    })),
            );
        }
        page.into_any_element()
    }

    pub(super) fn apply_profile(&mut self, p: &Profile, w: &mut Window, cx: &mut Context<Self>) {
        self.set("chip", p.target.chip.clone().unwrap_or_default(), w, cx);
        self.set("probe", p.probe.clone().unwrap_or_default(), w, cx);
        self.set(
            "speed",
            p.target.speed_khz.unwrap_or(4000).to_string(),
            w,
            cx,
        );
        self.set(
            "wire",
            match p.target.protocol {
                WireProtocol::Swd => "swd",
                WireProtocol::Jtag => "jtag",
            },
            w,
            cx,
        );
        self.set("base", p.base.clone(), w, cx);
        self.set(
            "erase",
            if p.options.skip_erase {
                "skip"
            } else if p.options.allow_erase_all {
                "full"
            } else {
                "automatic"
            },
            w,
            cx,
        );
        for (key, value) in [
            ("verify", p.options.verify),
            ("reset", p.options.reset_after),
            ("dry_run", p.options.dry_run),
            ("under_reset", p.target.connect_under_reset),
        ] {
            self.preferences.flags.insert(key.into(), value);
        }
        cx.notify();
    }
    fn store_profile(&mut self, w: &mut Window, cx: &mut Context<Self>) -> Result<()> {
        let mut p = self.snapshot(cx);
        anyhow::ensure!(!p.get("profile").trim().is_empty(), "配置名称不能为空");
        let target = p.target()?;
        let name = p.unique_profile_name(p.get("profile"));
        p.profiles.insert(
            name.clone(),
            Profile {
                target,
                probe: model::optional(p.get("probe")),
                options: p.options(),
                base: p.get("base").into(),
            },
        );
        p.fields.insert("profile".into(), name.clone());
        p.save(self.path.as_ref().context("无法读取设置路径，未保存配置")?)?;
        self.preferences = p;
        self.set("profile", name, w, cx);
        self.report("配置已保存", cx);
        Ok(())
    }
    pub(super) fn open_save_profile(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let p = self.snapshot(cx);
        let name = if p.get("chip").is_empty() {
            "烧录配置".to_owned()
        } else {
            p.get("chip").to_owned()
        };
        self.set("profile", name, w, cx);
        let owner = cx.entity().downgrade();
        if !w.has_active_dialog(cx) {
            self.focus_handle.focus(w, cx);
        }
        w.open_dialog(cx, move |d, _, cx| {
            let body = owner
                .update(cx, |s, cx| s.save_profile_sheet(cx))
                .unwrap_or_else(|_| div());
            let saving = owner.clone();
            d.p_0()
                .centered()
                .close_button(false)
                .w(px(400.))
                .rounded(px(16.))
                .child(body)
                .on_ok(move |_, w, cx| {
                    saving
                        .update(cx, |s, cx| match s.store_profile(w, cx) {
                            Ok(()) => true,
                            Err(e) => {
                                s.report_error(e.to_string(), cx);
                                false
                            }
                        })
                        .unwrap_or(false)
                })
        });
        let field = self.fields["profile"].clone();
        w.defer(cx, move |w, cx| {
            field.update(cx, |f, cx| f.focus(w, cx));
            w.dispatch_action(Box::new(gpui_kit::component::input::SelectAll), cx);
        });
    }
    fn save_profile_sheet(&self, cx: &mut Context<Self>) -> Div {
        let p = self.snapshot(cx);
        let blank = p.get("profile").trim().is_empty();
        let secondary = cx.theme().muted_foreground;
        let fact = |label: &'static str, value: String| {
            div()
                .flex()
                .gap(px(14.))
                .child(div().w(px(26.)).text_color(secondary).child(label))
                .child(value)
        };
        div()
            .p(px(24.))
            .flex()
            .flex_col()
            .gap(px(18.))
            .text_size(px(13.))
            .line_height(relative(1.2))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(div().font_weight(FontWeight::BOLD).child("保存烧录配置"))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(secondary)
                            .child("保存探针、目标芯片、连接参数与烧录选项。"),
                    ),
            )
            .child(
                Input::new(&self.fields["profile"])
                    .small()
                    .aria_label("配置名称"),
            )
            .child(
                div()
                    .p(px(12.))
                    .rounded(px(8.))
                    .bg(cx.theme().foreground.opacity(0.05))
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .text_size(px(12.))
                    .child(fact("目标", self.target_name(cx)))
                    .child(fact(
                        "接口",
                        format!("{} · {} kHz", p.get("wire").to_uppercase(), p.get("speed")),
                    ))
                    .child(fact(
                        "擦除",
                        match p.get("erase") {
                            "full" => "整片擦除",
                            "skip" => "跳过擦除",
                            _ => "按需擦除",
                        }
                        .into(),
                    )),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        bordered("cancel-profile", None, "取消")
                            .on_click(|_, w, cx| w.close_dialog(cx)),
                    )
                    .child(
                        self.button("save-profile", "保存", cx, |s, w, cx| {
                            match s.store_profile(w, cx) {
                                Ok(()) => w.close_dialog(cx),
                                Err(e) => s.report_error(e.to_string(), cx),
                            }
                        })
                        .primary()
                        .disabled(blank || self.is_busy()),
                    ),
            )
    }
}
