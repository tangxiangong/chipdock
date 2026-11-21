mod auxiliary;
pub use auxiliary::UpdatePresentation;
mod chrome;
mod colors;
mod commands;
#[cfg(target_os = "macos")]
mod commands_macos;
mod components;
mod file_dialogs;
mod firmware_history;
mod flash;
mod icons;
mod protocol;
mod records;
mod serial;
mod target;
mod tools;
mod windows;

use crate::{
    backend::{Command, Lane},
    workbench::Workbench,
};
use components::*;
use nsh_base::theme;
use nsh_core::prelude::*;
use std::time::Duration;

#[derive(Clone, PartialEq)]
struct StorageFailure {
    message: String,
    detail: String,
    recovery: String,
}

#[derive(Clone, Copy)]
pub(super) struct Store {
    data: Stored<Workbench>,
    settings_view: Prop<Option<Renderer<String>>>,
    auxiliary_view: Prop<Option<Renderer<String>>>,
    update_view: Prop<Option<Renderer<UpdatePresentation>>>,
    owner: Owner,
    revision: Signal<u64>,
    sidebar: Signal<bool>,
    composer: Signal<bool>,
    text_terminal: Signal<bool>,
    search: ElementRef,
    jump: Signal<Option<u64>>,
    modal: Signal<bool>,
    file_dialog: Signal<bool>,
    native_sheet: Signal<bool>,
    storage_failure: Signal<Option<StorageFailure>>,
    failure_visible: Signal<bool>,
    terminal_state: Stored<std::collections::BTreeMap<u64, serial::TerminalState>>,
    tool: Signal<Option<&'static str>>,
    tool_session: Signal<Option<u64>>,
    target_setup: Signal<bool>,
    target_chips: Signal<bool>,
    target_draft: Stored<Option<Signal<crate::model::Preferences>>>,
    probe_scan_target: Stored<Option<Option<Signal<crate::model::Preferences>>>>,
    command_context: Signal<commands::Context>,
    edit_actions: Signal<Option<commands::EditActions>>,
    window_queue: windows::WindowQueue,
    windows: Stored<std::collections::BTreeMap<String, nsh_core::element::desktop::WindowRef>>,
}
impl Store {
    fn read<R>(&self, f: impl FnOnce(&Workbench) -> R) -> R {
        self.revision.get();
        self.data.with(|s| f(s))
    }
    fn change<R>(&self, f: impl FnOnce(&mut Workbench) -> R) -> R {
        let result = self.data.with(f);
        self.revision.update(|n| *n += 1);
        result
    }
    fn field(&self, key: &str) -> String {
        self.read(|s| s.preferences.get(key).into())
    }
    fn flag(&self, key: &str) -> bool {
        self.read(|s| s.preferences.flag(key))
    }
    fn command(&self, make: impl FnOnce(&Workbench) -> Command) {
        self.change(|s| {
            let command = make(s);
            if matches!(command, Command::Scan) && !s.installing && !s.lane_busy(Lane::Probe) {
                self.probe_scan_target
                    .replace(Some(self.target_draft.get()));
            }
            s.submit(command)
        })
    }
    fn locked(&self, key: &str) -> bool {
        self.read(|s| {
            s.installing
                || if [
                    "chip", "probe", "wire", "speed", "base", "firmware", "erase",
                ]
                .contains(&key)
                {
                    s.lane_busy(Lane::Probe)
                } else if ["port", "baud", "bits", "stop", "parity", "flow"].contains(&key) {
                    s.serial_connected() || s.serial_busy()
                } else {
                    false
                }
        })
    }
    fn save(&self) {
        self.change(|s| match s.save() {
            Ok(()) => s.report("设置已保存"),
            Err(e) => s.error(e),
        })
    }
    fn save_before_exit(&self) -> bool {
        self.change(|s| match s.save() {
            Ok(()) => true,
            Err(error) => {
                s.error(error);
                false
            }
        })
    }
    fn report(&self, error: impl std::fmt::Display) {
        self.change(|s| s.error(error))
    }
    fn queue_storage_error(&self, error: impl std::fmt::Display) {
        let detail = format!("{error:#}");
        let response = chipdock_core::error::AppError::StorageFailure { detail }.to_response();
        let detail = response.detail.unwrap_or_default();
        self.change(|s| {
            s.error(&detail);
            s.message = detail.clone();
        });
        self.storage_failure.set(Some(StorageFailure {
            message: response.message,
            detail,
            recovery: response.recovery,
        }));
    }
    fn spawn(&self, future: impl std::future::Future<Output = ()> + 'static) {
        self.owner.run(|| {
            spawn(future);
        });
    }
    fn pick(&self, key: &'static str) {
        if self.file_dialog.get_untracked() {
            return;
        }
        self.file_dialog.set(true);
        let store = *self;
        self.spawn(async move {
            let kind = if key == "firmware" {
                file_dialogs::OpenKind::DocumentFirmware
            } else {
                file_dialogs::OpenKind::File
            };
            store
                .native_sheet
                .set(cfg!(target_os = "macos") && key == "firmware");
            let result = file_dialogs::open_path(kind).await;
            store.native_sheet.set(false);
            store.file_dialog.set(false);
            match result {
                Ok(Some(path)) => store.change(|s| {
                    let path = path.to_string_lossy();
                    if key == "firmware" {
                        s.open_recent(&path)
                    } else {
                        s.set(key, path.into_owned())
                    }
                }),
                Ok(None) => {}
                Err(e) => store.report(e),
            }
        });
    }
    fn export(&self, name: &str, bytes: Vec<u8>) {
        self.export_as(name, bytes, file_dialogs::ExportKind::Save);
    }
    fn export_as(&self, name: &str, bytes: Vec<u8>, kind: file_dialogs::ExportKind) {
        if self.file_dialog.get_untracked() {
            return;
        }
        self.file_dialog.set(true);
        let store = *self;
        let name = name.to_owned();
        self.spawn(async move {
            store
                .native_sheet
                .set(cfg!(target_os = "macos") && !matches!(kind, file_dialogs::ExportKind::Save));
            let result = file_dialogs::save_path(name, kind).await;
            store.native_sheet.set(false);
            store.file_dialog.set(false);
            match result {
                Ok(Some(path)) => {
                    store.command(|_| Command::Export(path.to_string_lossy().into_owned(), bytes))
                }
                Ok(None) => {}
                Err(e) => store.report(e),
            }
        });
    }
}

pub fn workbench(
    props: crate::WorkbenchProps,
    cx: Scope<crate::WorkbenchEvent>,
) -> impl IntoElement {
    let store = Store {
        data: stored(Workbench::new()),
        settings_view: props.render_settings,
        auxiliary_view: props.render_auxiliary,
        update_view: props.render_update,
        owner: Owner::current(),
        revision: signal(0),
        sidebar: signal(true),
        composer: signal(true),
        text_terminal: signal(false),
        search: element_ref(),
        jump: signal(None),
        modal: signal(false),
        file_dialog: signal(false),
        native_sheet: signal(false),
        storage_failure: signal(None),
        failure_visible: signal(false),
        terminal_state: stored(std::collections::BTreeMap::new()),
        tool: signal(None),
        tool_session: signal(None),
        target_setup: signal(false),
        target_chips: signal(false),
        target_draft: stored(None),
        probe_scan_target: stored(None),
        command_context: signal(commands::Context::main()),
        edit_actions: signal(None),
        window_queue: windows::WindowQueue::new(),
        windows: stored(std::collections::BTreeMap::new()),
    };
    provide_context(components::MainWindowModal(store.modal, store.file_dialog));
    provide_context(components::NativeMenuActivity(signal(0usize)));
    let scan_revision = stored(0);
    effect(move || {
        let revision = store.read(|state| state.probe_scan_revision);
        if revision == scan_revision.get() {
            return;
        }
        scan_revision.replace(revision);
        match store.probe_scan_target.with(Option::take) {
            Some(Some(draft)) => {
                if let Some(mut preferences) = draft.try_get()
                    && let Some(probe) = store.read(|state| {
                        crate::workbench::scanned_probe_selection(
                            preferences.get("probe"),
                            &state.probes,
                        )
                    })
                {
                    preferences.fields.insert("probe".into(), probe);
                    draft.set(preferences);
                }
            }
            _ => store.change(Workbench::select_scanned_probe),
        }
    });
    let connecting = stored(false);
    effect(move || {
        if store.read(Workbench::target_connecting) {
            connecting.replace(true);
        } else if connecting.get() {
            connecting.replace(false);
            if !store.target_setup.get_untracked()
                && store.read(|state| !state.target_connected() && !state.chips.is_empty())
            {
                store.target_chips.set(true);
                store.target_setup.set(true);
            }
        }
    });
    match nsh_base::theme_overrides(include_str!("../../theme.json")) {
        Ok(tokens) => set_tokens(tokens),
        Err(error) => store.report(format!("主题配置无效：{error}")),
    }
    let mode = signal("flash");
    cx.methods(WorkbenchApi { store });
    let main = nsh_core::element::desktop::main_window();
    let mut attributes = nsh_core::desktop::WindowAttributes::default();
    attributes.min_size = Some(Some(Size {
        width: px(900.),
        height: px(652.),
    }));
    attributes.close_on_press = Some(false);
    main.set(attributes);
    let closing = main.clone();
    main.on_close_press(move |_| {
        if store.file_dialog.get_untracked() {
            return;
        }
        if store.modal.get_untracked() {
            if store.tool.get_untracked().is_some() {
                tools::close(store);
            }
            return;
        }
        if store.read(Workbench::can_quit) {
            if store.save_before_exit() {
                closing.close();
            }
        } else {
            store.report("任务或采集正在运行，请先停止再退出");
        }
    });
    let appearance = store.read(|s| s.preferences.theme.clone());
    set_appearance(&appearance);
    set_interval(Duration::from_millis(100), move || {
        let changed = store.data.with(Workbench::tick);
        if changed {
            store.revision.update(|n| *n += 1);
        }
        let (restart, plot) = store.data.with(|s| (s.restart, s.plot_request.take()));
        if let Some(id) = plot {
            auxiliary::open(store, auxiliary::Panel::Plot(id));
        }
        if restart {
            nsh_core::element::desktop::restart();
        }
    });
    nsh_core::element::desktop::on_app_quit(move || async move {
        if let Some(result) = store.data.try_with(|s| s.save())
            && let Err(error) = result
        {
            eprintln!("保存设置失败：{error:#}");
        }
        match nsh_core::element::blocking(chipdock_service::serial::serial_shutdown).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => eprintln!("关闭串口失败：{error}"),
            Err(error) => eprintln!("关闭串口任务失败：{error}"),
        }
    });
    spawn(async move {
        loop {
            let paths = next_open_files().await;
            if let Some(path) = paths.first()
                && !store.read(|s| s.lane_busy(Lane::Probe))
                && !store.modal.get_untracked()
                && !store.file_dialog.get_untracked()
            {
                store.change(|s| s.open_recent(&path.to_string_lossy()));
                mode.set("flash");
            }
        }
    });
    let announced = stored(String::new());
    effect(move || {
        let message = store.read(|s| s.message.clone());
        if announced.get() != message {
            announced.replace(message.clone());
            if message.starts_with("[错误]") || message.ends_with("完成") {
                announce_for_accessibility(message);
            }
        }
    });
    shortcuts(store, mode);
    effect(move || {
        if store.modal.get() || store.file_dialog.get() || store.failure_visible.get() {
            return;
        }
        if let Some(failure) = store.storage_failure.get() {
            store.storage_failure.set(None);
            store.failure_visible.set(true);
            store.modal.set(true);
            store.spawn(async move {
                let result = nsh_base::alert_dialog()
                    .title(failure.message)
                    .description(
                        [failure.detail, failure.recovery]
                            .into_iter()
                            .filter(|part| !part.is_empty())
                            .collect::<Vec<_>>()
                            .join("\n\n"),
                    )
                    .cancel("ok", "确定")
                    .action("logs", "查看日志")
                    .show()
                    .await;
                store.modal.set(false);
                store.failure_visible.set(false);
                match result {
                    Ok(result) if result.action.as_deref() == Some("logs") => {
                        auxiliary::open(store, auxiliary::Panel::Logs)
                    }
                    Err(error) => store.report(error),
                    _ => {}
                }
            });
        }
    });
    provide_context((store, mode));
    let layout = computed(move || {
        (
            mode.get(),
            store.sidebar.get(),
            store.composer.get(),
            store.modal.get() || store.file_dialog.get(),
            store.target_setup.get(),
            store.field("firmware").is_empty(),
        )
    });
    effect(move || {
        let (mode, sidebar, composer, modal, target_setup, empty) = layout.get();
        cx.emit(crate::WorkbenchEvent::StateChanged(
            mode.into(),
            sidebar,
            composer,
            modal,
            target_setup,
            empty,
        ));
    });
    effect(move || {
        cx.emit(crate::WorkbenchEvent::ToolChange(
            store.tool.get().map(str::to_owned),
            store.native_sheet.get(),
        ))
    });
    view()
        .text_size(body_size)
        .line_height(nsh_core::LineHeight::Multiple(1.2))
        .child(props.children)
        .child(store.window_queue.host())
        .on_file_drop(move |event: &nsh_core::prelude::FileDropEvent| {
            if store.modal.get_untracked()
                || store.file_dialog.get_untracked()
                || store.read(|s| s.lane_busy(Lane::Probe))
            {
                return;
            }
            if let Some(uri) = event.uris.first() {
                match crate::model::dropped_file(uri) {
                    Ok(path) => {
                        store.change(|s| s.open_recent(&path.to_string_lossy()));
                        mode.set("flash");
                    }
                    Err(error) => store.report(error),
                }
            }
        })
}

fn set_appearance(theme: &str) {
    set_color_scheme(match theme {
        "light" => nsh_core::platform::Appearance::Light,
        "dark" => nsh_core::platform::Appearance::Dark,
        _ => nsh_core::platform::Appearance::System,
    });
    #[cfg(target_os = "macos")]
    {
        let appearance = match theme {
            "light" => Some(false),
            "dark" => Some(true),
            _ => None,
        };
        // NSH applies appearance to the root view and auxiliary windows; the
        // main NSWindow also supplies the appearance of attached sheet curtains.
        drop(run_on_main_thread(move || {
            use nsh_platform_macos::{
                objc2::MainThreadMarker,
                objc2_app_kit::{
                    NSAppearance, NSAppearanceCustomization, NSAppearanceNameAqua,
                    NSAppearanceNameDarkAqua, NSApplication,
                },
            };
            let Some(marker) = MainThreadMarker::new() else {
                return;
            };
            let native = appearance.and_then(|dark| {
                NSAppearance::appearanceNamed(unsafe {
                    if dark {
                        NSAppearanceNameDarkAqua
                    } else {
                        NSAppearanceNameAqua
                    }
                })
            });
            for window in NSApplication::sharedApplication(marker).windows().iter() {
                if window.title().to_string() == "ChipDock" {
                    window.setAppearance(native.as_deref());
                }
            }
        }));
    }
}
fn modal_backdrop(store: Store) -> AdaptiveColor {
    if store.native_sheet.get() {
        theme().background.opacity(0.)
    } else {
        theme()
            .color("scrim")
            .unwrap_or(theme().foreground.opacity(0.4))
    }
}
fn shortcuts(store: Store, mode: Signal<&'static str>) {
    commands::install(store, mode);
}

#[cfg(debug_assertions)]
pub(crate) fn preview_available(
    feed: crate::update::Feed,
    render_update: Prop<Option<Renderer<UpdatePresentation>>>,
) -> View {
    auxiliary::preview_available(feed, render_update)
}

#[derive(Debug, Clone, PartialEq, Props)]
pub struct WorkbenchIconProps {
    /// Name in ChipDock's compiled SVG catalog.
    pub name: Prop<SharedString>,
    /// Description for an icon that carries meaning without adjacent text.
    pub accessibility_label: Prop<Option<SharedString>>,
}

/// ChipDock's original vector artwork, inheriting its foreground from the surrounding control.
#[component]
pub fn WorkbenchIcon(props: WorkbenchIconProps) -> impl IntoElement {
    svg(move || icons::source(&props.name.get()))
        .optional_label(props.accessibility_label)
        .size(px(16.))
        .flex_shrink_0()
}

/// Opens the native firmware chooser when the probe lane is available.
#[component]
pub fn ChooseFirmware() -> impl IntoElement {
    let (store, _) = workbench_context();
    labeled_button(
        "folder",
        "选择文件…",
        false,
        move || store.locked("firmware"),
        move || store.pick("firmware"),
    )
}

/// Recent firmware files for the current session.
#[component]
pub fn FirmwareHistory() -> impl IntoElement {
    let (store, _) = workbench_context();
    firmware_history::page(store)
}

/// Commands on the workbench that owns the mounted screens.
pub struct WorkbenchApi {
    store: Store,
}

#[methods]
impl WorkbenchApi {
    /// Dismisses the active serial tool and releases its modal guard.
    pub fn close_tool(&self) {
        tools::close(self.store);
    }
}
fn workbench_context() -> (Store, Signal<&'static str>) {
    use_context().expect("workbench content requires WorkbenchHost")
}
pub(crate) fn register_regions(registry: &mut Registry) {
    registry
        .register::<WorkbenchIcon>()
        .register::<ChooseFirmware>()
        .register::<FirmwareHistory>();
    flash::register(registry);
    target::register(registry);
    serial::register(registry);
    records::register(registry);
    chrome::register(registry);
    tools::register(registry);
    auxiliary::register(registry);
}
