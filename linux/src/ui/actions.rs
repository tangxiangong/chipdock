use super::*;

impl Workbench {
    pub(super) fn pick_capture_directory(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("选择采集目录".into()),
        });
        cx.spawn(async move |view, cx| {
            if let Ok(Ok(Some(paths))) = paths.await
                && let Some(directory) = paths.first()
            {
                let directory = directory.clone();
                let _ = view.update_in(cx, |s, w, cx| {
                    let extension = s.fields["capture_format"].read(cx).value().to_string();
                    let path = directory.join(format!(
                        "ChipDock-{}.{}",
                        chrono::Local::now().format("%Y%m%d-%H%M%S"),
                        extension
                    ));
                    s.set("capture_path", path.to_string_lossy().into_owned(), w, cx);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub(super) fn disconnect_target(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.connected_target = None;
        self.target_info = None;
        self.memory = None;
        self.chips.clear();
        self.layout.clear();
        self.memory_output
            .update(cx, |s, cx| s.set_value("", w, cx));
        self.report("已断开", cx);
    }
    pub(super) fn clear_terminal(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if let Some(session) = self.sessions.iter_mut().find(|s| Some(s.id) == self.active) {
            session.cleared_through = session.cursor;
            session.frames.clear();
        }
        self.render_terminal(w, cx);
        if self.preferences.flag("reset_on_clear") {
            self.serial_action("resetCounters", cx);
        }
    }

    pub(super) fn import_editor(&mut self, key: &'static str, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("导入 JSON".into()),
        });
        cx.spawn(async move |view, cx| {
            if let Ok(Ok(Some(paths))) = paths.await
                && let Some(path) = paths.first()
            {
                let path = path.to_string_lossy().into_owned();
                let _ = view.update(cx, |s, cx| s.submit(Command::Import(key.into(), path), cx));
            }
        })
        .detach();
    }
    pub(super) fn export_bytes(&mut self, name: &str, bytes: Vec<u8>, cx: &mut Context<Self>) {
        let directory = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let path = cx.prompt_for_new_path(&directory, Some(name));
        cx.spawn(async move |view, cx| {
            if let Ok(Ok(Some(path))) = path.await {
                let _ = view.update(cx, |s, cx| {
                    s.submit(
                        Command::Export(path.to_string_lossy().into_owned(), bytes),
                        cx,
                    )
                });
            }
        })
        .detach();
    }

    pub(super) fn pick(&mut self, key: &'static str, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |view, cx| {
            if let Ok(Ok(Some(paths))) = paths.await
                && let Some(path) = paths.first()
            {
                let path = path.to_string_lossy().into_owned();
                let _ = view.update_in(cx, |s, w, cx| {
                    s.set(key, path, w, cx);
                    cx.notify();
                });
            }
        })
        .detach();
    }
    pub(super) fn serial_action(&mut self, action: &str, cx: &mut Context<Self>) {
        let p = self.snapshot(cx);
        let result = (|| -> Result<Command> {
            let id = self.active.context("请先新建串口会话")?;
            let mut value = crate::serial::request(&p, action)?;
            if action == "export" {
                value["after"] = json!(
                    self.sessions
                        .iter()
                        .find(|s| s.id == id)
                        .map_or(0, |s| s.cleared_through)
                );
            }
            Ok(Command::Workbench(id, value))
        })();
        match result {
            Ok(command) => self.submit(command, cx),
            Err(e) => self.report_error(format!("{e:#}"), cx),
        }
    }
}
