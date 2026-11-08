use super::*;

impl Workbench {
    pub(super) fn settings_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut page = div()
            .flex()
            .flex_col()
            .gap_4()
            .child(div().text_xl().child("设置"))
            .child(
                div().child("外观").flex().gap_2().children(
                    [("system", "跟随系统"), ("light", "浅色"), ("dark", "深色")]
                        .into_iter()
                        .map(|(mode, label)| {
                            Button::new(mode)
                                .label(label)
                                .selected(self.preferences.theme == mode)
                                .on_click(cx.listener(move |s, _, w, cx| {
                                    s.preferences.theme = mode.into();
                                    match mode {
                                        "light" => Theme::change(ThemeMode::Light, Some(w), cx),
                                        "dark" => Theme::change(ThemeMode::Dark, Some(w), cx),
                                        _ => Theme::sync_system_appearance(Some(w), cx),
                                    }
                                    s.save(w, cx);
                                }))
                        }),
                ),
            )
            .child(
                div().child("CLI / MCP 本地控制").flex().gap_2().children(
                    [(0, "关闭"), (1, "只读"), (2, "允许控制")]
                        .into_iter()
                        .map(|(mode, label)| {
                            Button::new(("ipc", mode as usize))
                                .label(label)
                                .selected(self.preferences.ipc == mode)
                                .on_click(cx.listener(move |s, _, _, cx| {
                                    s.submit(Command::Ipc(mode), cx);
                                }))
                        }),
                ),
            )
            .child("新会话烧录默认值")
            .child(self.flag("default_verify", "默认写入后校验", cx))
            .child(self.flag("default_reset", "默认完成后复位", cx))
            .child(self.button("save", "保存设置", cx, |s, w, cx| s.save(w, cx)))
            .child(format!(
                "ChipDock {} · GPUI Kit · Rust",
                env!("CARGO_PKG_VERSION")
            ))
            .child(
                Button::new("source")
                    .label("源代码")
                    .on_click(|_, _, cx| cx.open_url("https://github.com/tangxiangong/chipdock")),
            )
            .child(
                Button::new("releases")
                    .label("发布记录与下载")
                    .on_click(|_, _, cx| {
                        cx.open_url("https://github.com/tangxiangong/chipdock/releases")
                    }),
            )
            .child("MIT OR Apache-2.0")
            .child(self.flag("auto_update", "自动检查更新（每 24 小时）", cx))
            .child(self.flag("auto_download", "自动下载已验证的更新", cx))
            .child(self.flag("auto_install", "下载后空闲时自动安装并重启", cx))
            .child(
                Button::new("cancel-download")
                    .label("取消下载")
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.worker
                            .cancel_update
                            .store(true, std::sync::atomic::Ordering::Relaxed);
                        s.report("正在取消下载…", cx);
                    })),
            )
            .child(self.button("check-update", "检查更新", cx, |s, _, cx| {
                s.submit(Command::CheckUpdate, cx)
            }));
        if let Some(update) = &self.update {
            page = page
                .child(format!("新版本 {}", update.version))
                .child(update.notes.clone());
            let available = update.clone();
            page = page.child(self.button(
                "download-update",
                "下载更新",
                cx,
                move |s, _, cx| s.submit(Command::DownloadUpdate(available.clone()), cx),
            ));
            if let Some(path) = &self.downloaded {
                let path = path.clone();
                let available = update.clone();
                page = page.child(self.button(
                    "install-update",
                    "安装并重新启动",
                    cx,
                    move |s, w, cx| {
                        let _ = w;
                        s.request_install(path.clone(), available.clone(), cx);
                    },
                ));
            }
        }
        page.into_any_element()
    }
}
