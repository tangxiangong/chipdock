use super::*;

impl Workbench {
    pub(super) fn settings_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut page = div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                gpui_kit::component::TitleBar::new().h(px(32.)).child(
                    div()
                        .w_full()
                        .mr(px(80.))
                        .text_center()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(if self.settings_tab == "general" {
                            "通用"
                        } else {
                            "关于"
                        }),
                ),
            )
            .child(
                div()
                    .h(px(56.))
                    .flex_shrink_0()
                    .flex()
                    .justify_center()
                    .items_center()
                    .gap_1()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .children(
                        [("general", "通用", "gear"), ("about", "关于", "info")]
                            .into_iter()
                            .map(|(tab, label, icon)| {
                                native_button(tab)
                                    .tooltip(label)
                                    .ghost()
                                    .w(px(56.))
                                    .h(px(48.))
                                    .rounded_lg()
                                    .selected(self.settings_tab == tab)
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .items_center()
                                            .gap_1()
                                            .child(icons::icon(icon).size(px(22.)))
                                            .child(div().text_size(px(12.)).child(label)),
                                    )
                                    .on_click(cx.listener(move |s, _, _, cx| {
                                        s.settings_tab = tab;
                                        cx.notify();
                                    }))
                            }),
                    ),
            );
        if self.settings_tab == "general" {
            let owner = cx.entity().downgrade();
            let theme = self.preferences.theme.clone();
            page=page.child(div().p_5().flex().flex_col().gap_5()
                .child(div().flex().flex_col().gap_2().child(div().px_2().font_weight(FontWeight::SEMIBOLD).child("外观"))
                    .child(div().h(px(38.)).px_3().rounded_lg().bg(group_bg(cx)).flex().items_center().justify_between().child("主题")
                        .child(native_button("theme-menu").small().ghost().label(match theme.as_str(){"light"=>"浅色","dark"=>"深色",_=>"跟随系统"}).child(icons::icon("select")).dropdown_menu(move |mut m,_,_|{
                            for (mode,label) in [("system","跟随系统"),("light","浅色"),("dark","深色")]{let owner=owner.clone();m=m.item(PopupMenuItem::new(label).checked(theme==mode).on_click(move |_,w,cx|{let _=owner.update(cx,|s,cx|{s.preferences.theme=mode.into();match mode{"light"=>Theme::change(ThemeMode::Light,Some(w),cx),"dark"=>Theme::change(ThemeMode::Dark,Some(w),cx),_=>Theme::sync_system_appearance(Some(w),cx)}components::native_accent(cx);s.save(w,cx);});}));}m
                        }))))
                .child(div().flex().flex_col().gap_2().child(div().px_2().font_weight(FontWeight::SEMIBOLD).child("新会话的烧录默认值"))
                    .child(div().px_3().rounded_lg().bg(group_bg(cx)).children([("default_verify","写入后校验"),("default_reset","完成后复位")].into_iter().map(|(key,label)|{
                        div().h(px(38.)).flex().items_center().justify_between().child(label)
                            .child(gpui_kit::component::switch::Switch::new(key).small().checked(self.preferences.flag(key)).on_click(cx.listener(move |s,value,w,cx|{s.preferences.flags.insert(key.into(),*value);s.save(w,cx);})))
                    })))));
        } else {
            let mut about = div()
                .id("about-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .py_5()
                .flex()
                .flex_col()
                .items_center()
                .gap_3()
                .child(img("app-icon.png").size(px(64.)))
                .child(
                    div()
                        .text_size(px(18.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("ChipDock"),
                )
                .child(
                    div()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("版本 {}", env!("CARGO_PKG_VERSION"))),
                )
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .items_center()
                        .child(
                            native_button("source")
                                .small()
                                .ghost()
                                .label("源代码")
                                .text_color(rgb(0x007aff))
                                .on_click(|_, _, cx| {
                                    cx.open_url("https://github.com/tangxiangong/chipdock")
                                }),
                        )
                        .child(
                            native_button("releases")
                                .small()
                                .ghost()
                                .label("发布记录")
                                .text_color(rgb(0x007aff))
                                .on_click(|_, _, cx| {
                                    cx.open_url("https://github.com/tangxiangong/chipdock/releases")
                                }),
                        )
                        .child(self.button("check-update", "检查更新…", cx, |s, _, cx| {
                            s.submit(Command::CheckUpdate, cx)
                        })),
                )
                .child(self.flag("auto_update", "自动检查更新", cx))
                .child(self.flag("auto_download", "自动下载更新", cx))
                .child(self.flag("auto_install", "下载后空闲时安装并重启", cx))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(cx.theme().muted_foreground)
                        .child("MIT OR Apache-2.0"),
                );
            if let Some(update) = &self.update {
                about = about
                    .child(format!("新版本 {}", update.version))
                    .child(update.notes.clone());
                let available = update.clone();
                about = about.child(self.button(
                    "download-update",
                    "下载更新",
                    cx,
                    move |s, _, cx| s.submit(Command::DownloadUpdate(available.clone()), cx),
                ));
                if let Some(path) = &self.downloaded {
                    let path = path.clone();
                    let available = update.clone();
                    about = about.child(self.button(
                        "install-update",
                        "安装并重新启动",
                        cx,
                        move |s, _, cx| s.request_install(path.clone(), available.clone(), cx),
                    ));
                }
                about = about.child(
                    native_button("cancel-download")
                        .small()
                        .label("取消下载")
                        .on_click(cx.listener(|s, _, _, cx| {
                            s.worker
                                .cancel_update
                                .store(true, std::sync::atomic::Ordering::Relaxed);
                            s.report("正在取消下载…", cx);
                        })),
                );
            }
            page = page.child(about);
        }
        page.into_any_element()
    }
}
fn group_bg(cx: &App) -> Hsla {
    rgb(if cx.theme().is_dark() {
        0x252525
    } else {
        0xf7f7f7
    })
    .into()
}
