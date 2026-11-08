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
                let p = s.snapshot(cx);
                if p.get("profile").trim().is_empty() {
                    s.report_error("配置名称不能为空", cx);
                    return;
                }
                match p.target() {
                    Ok(target) => {
                        let name = s.preferences.unique_profile_name(p.get("profile"));
                        s.set("profile", name.clone(), w, cx);
                        s.preferences.profiles.insert(
                            name,
                            Profile {
                                target,
                                probe: model::optional(p.get("probe")),
                                options: p.options(),
                                base: p.get("base").into(),
                            },
                        );
                        s.save(w, cx);
                    }
                    Err(e) => s.report_error(e.to_string(), cx),
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
                        Button::new(("profile", index))
                            .label(name)
                            .on_click(cx.listener(move |s, _, w, cx| {
                                s.set("chip", p.target.chip.clone().unwrap_or_default(), w, cx);
                                s.set("probe", p.probe.clone().unwrap_or_default(), w, cx);
                                s.set(
                                    "speed",
                                    p.target.speed_khz.unwrap_or(4000).to_string(),
                                    w,
                                    cx,
                                );
                                s.set(
                                    "wire",
                                    match p.target.protocol {
                                        WireProtocol::Swd => "swd",
                                        WireProtocol::Jtag => "jtag",
                                    },
                                    w,
                                    cx,
                                );
                                s.set("base", p.base.clone(), w, cx);
                                s.set(
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
                                    s.preferences.flags.insert(key.into(), value);
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new(("remove-profile", index))
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
                        Button::new(("reveal-recent", index))
                            .label("显示文件")
                            .on_click(move |_, _, cx| {
                                cx.reveal_path(std::path::Path::new(&reveal))
                            }),
                    )
                    .child(
                        Button::new(("remove-recent", index))
                            .label("从历史移除")
                            .on_click(cx.listener(move |s, _, w, cx| {
                                s.preferences.recent.retain(|p| p != &remove);
                                s.save(w, cx);
                            })),
                    ),
            );
            page = page.child(Button::new(("recent", index)).label(path.clone()).on_click(
                cx.listener(move |s, _, w, cx| {
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
                }),
            ));
        }
        page.into_any_element()
    }
}
