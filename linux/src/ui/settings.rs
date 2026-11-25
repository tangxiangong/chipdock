//! Settings retain the macOS workbench's title bar, icon tabs and grouped rows.
use super::*;
#[cfg(not(target_os = "macos"))]
use nsh_base::{SelectEntry, SelectItem};
use nsh_core::element::ImageSrc;

pub(super) fn page(store: Store) -> View {
    let saved = stored(settings_key(store));
    effect(move || {
        let key = settings_key(store);
        if saved.get() != key {
            saved.replace(key);
            store.save();
        }
    });

    provide_context(store);
    view().flex_1().min_h_0().child(
        store
            .settings_view
            .get()
            .expect("App.tsx supplies the settings page")
            .render("settings", env!("CARGO_PKG_VERSION").to_owned())
            .flex_1()
            .min_h_0(),
    )
}

fn settings_key(store: Store) -> (String, bool, bool, bool, bool, bool) {
    store.read(|s| {
        (
            s.preferences.theme.clone(),
            s.preferences.flag("default_verify"),
            s.preferences.flag("default_reset"),
            s.preferences.flag("auto_update"),
            s.preferences.flag("auto_download"),
            s.preferences.flag("auto_install"),
        )
    })
}

fn appearance_menu(store: Store) -> impl IntoElement {
    #[cfg(target_os = "macos")]
    {
        native_choice_menu(
            theme_trigger(store)
                .label("主题")
                .accessibility_value(move || {
                    Some(nsh_core::AccessibilityValue {
                        text: Some(std::sync::Arc::<str>::from(option_label(
                            &store.read(|state| state.preferences.theme.clone()),
                        ))),
                        ..Default::default()
                    })
                }),
            || false,
            move || match store.read(|state| state.preferences.theme.clone()).as_str() {
                "light" => 1,
                "dark" => 2,
                _ => 0,
            },
            false,
            || {
                ["跟随系统", "浅色", "深色"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect()
            },
            move |index| {
                if let Some(value) = ["system", "light", "dark"].get(index) {
                    super::set_appearance(value);
                    store.change(|state| state.preferences.theme = (*value).to_owned());
                }
            },
        )
    }
    #[cfg(not(target_os = "macos"))]
    {
        nsh_base::select()
            .items(vec![
                SelectItem::new("system", "跟随系统"),
                SelectItem::new("light", "浅色"),
                SelectItem::new("dark", "深色"),
            ])
            .value(move || {
                Some(SharedString::from(
                    store.read(|s| s.preferences.theme.clone()),
                ))
            })
            .on_change(move |next: &Option<SharedString>| {
                if let Some(next) = next {
                    super::set_appearance(next);
                    store.change(|s| s.preferences.theme = next.to_string());
                }
            })
            .accessibility_label("主题")
            .popup_style(|style| popup(style).w(px(100.)).min_w(px(100.)))
            .item_class("rounded-[8px] focus:bg-menu-highlight focus:text-white")
            .render_item(move |item: &SelectEntry| {
                let value = item.value.clone();
                view()
                    .h(px(24.))
                    .px(px(8.))
                    .flex_row()
                    .items_center()
                    .gap(px(2.))
                    .child(
                        text(move || {
                            if store
                                .read(|state| state.preferences.theme.as_str() == value.as_ref())
                            {
                                "✓"
                            } else {
                                ""
                            }
                        })
                        .w(px(16.))
                        .text_center()
                        .text_size(body_size),
                    )
                    .child(text(item.title.clone()).text_size(body_size))
            })
            .trigger(theme_trigger(store))
    }
}

fn theme_trigger(store: Store) -> View {
    view()
        .h(px(24.))
        .px_1()
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            text(move || option_label(&store.read(|s| s.preferences.theme.clone())))
                .text_size(body_size),
        )
        .child(
            view()
                .size(px(20.))
                .rounded_full()
                .items_center()
                .justify_center()
                .bg(move || theme().color("control").unwrap_or(theme().secondary))
                .child(
                    svg(ImageSrc::svg_markup(include_str!(
                        "../../assets/icons/select.svg"
                    )))
                    .size(px(12.)),
                ),
        )
}

fn preference_switch(
    store: Store,
    key: &'static str,
    label: &'static str,
    separator: bool,
) -> View {
    view()
        .h(px(37.))
        .flex_row()
        .items_center()
        .justify_between()
        .when(separator, |v| {
            v.border_b_1()
                .border_color(move || theme().foreground.opacity(0.08))
        })
        .child(text(label).text_size(body_size))
        .child(
            nsh_base::switch()
                .checked(move || store.flag(key))
                .on_change(move |next: &bool| store.change(|s| s.flag(key, *next)))
                .accessibility_label(label)
                .track_style(|s| {
                    s.w(px(36.))
                        .h(px(16.))
                        .p(px(1.5))
                        .rounded_full()
                        .bg(move || theme().foreground.opacity(0.18))
                        .checked(|s| s.bg(move || theme().primary))
                        .transition_colors()
                        .duration(INSTANT)
                })
                .thumb_style(|s| {
                    s.w(px(21.))
                        .h(px(13.))
                        .rounded_full()
                        .bg(move || theme().primary_foreground)
                        .checked(|s| s.translate_x(px(12.)))
                        .transition_transform()
                        .duration(FAST)
                }),
        )
}

fn app_icon() -> ImageSrc {
    static SOURCE: std::sync::LazyLock<SharedString> = std::sync::LazyLock::new(|| {
        use base64::Engine as _;
        #[cfg(target_os = "macos")]
        let bytes: &[u8] = include_bytes!(env!("CHIPDOCK_MACOS_ICON_PNG"));
        #[cfg(not(target_os = "macos"))]
        let bytes: &[u8] = include_bytes!("../../../windows/ChipDock/Assets/ChipDock.png");
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        )
        .into()
    });
    ImageSrc::from(SOURCE.clone())
}

fn app_icon_extent(about: bool) -> Points {
    #[cfg(target_os = "macos")]
    let extent = if about { 76. } else { 64. };
    #[cfg(not(target_os = "macos"))]
    let extent = if about { 62. } else { 52. };
    px(extent)
}

fn automatic_update_toggle(store: Store, combined: bool) -> impl IntoElement {
    let label = if combined {
        "自动下载并安装更新"
    } else {
        "自动检查更新"
    };
    let checked = move || {
        if combined {
            store.flag("auto_download") && store.flag("auto_install")
        } else {
            store.flag("auto_update")
        }
    };
    nsh_base::checkbox()
        .checked(checked)
        .indeterminate(move || {
            combined && store.flag("auto_download") != store.flag("auto_install")
        })
        .on_change(move |enabled: &bool| {
            store.change(|s| {
                if combined {
                    s.flag("auto_download", *enabled);
                    s.flag("auto_install", *enabled);
                } else {
                    s.flag("auto_update", *enabled);
                }
            })
        })
        .disabled(move || store.read(|s| s.installing))
        .accessibility_label(label)
        .flex_row()
        .h(px(16.5))
        .items_center()
        .gap(px(4.75))
        .indicator_style(|s| {
            s.size(px(16.))
                .rounded(px(4.))
                .items_center()
                .justify_center()
                .bg(move || theme().color("checkbox").unwrap_or(theme().secondary))
                .checked(|s| s.bg(move || theme().primary))
                .indeterminate(|s| s.bg(move || theme().primary))
        })
        .indicator(
            text(move || {
                if checked() {
                    "✓"
                } else if combined && store.flag("auto_download") != store.flag("auto_install") {
                    "—"
                } else {
                    ""
                }
            })
            .text_size(caption_size)
            .text_color(move || theme().primary_foreground),
        )
        .child(text(label).text_size(body_size))
}

/// The manual check owns its presentation; background checks only update domain state.
pub(super) fn manual_check(store: Store) {
    if store.read(|s| s.lane_busy(Lane::Update)) {
        return;
    }
    store.command(|_| Command::CheckUpdate);
    #[cfg(target_os = "macos")]
    store.spawn(async move {
        match system_update_appearance::initial().await {
            Ok(system_dark) => open_update_window(store, system_dark),
            Err(error) => store.report(error),
        }
    });
    #[cfg(not(target_os = "macos"))]
    open_update_window(store, dark());
}

fn open_update_window(store: Store, system_dark: bool) {
    use nsh_core::element::desktop::window;
    let opened = super::super::windows::open(
        store,
        "software-update",
        window()
            .title("软件更新".to_owned())
            .parent("main")
            .restore_bounds(false)
            .default_position(nsh_core::desktop::WindowPosition::Center)
            .default_size(Size {
                width: px(400.),
                height: px(146.),
            })
            .attributes(move || update_window_attributes(store)),
        move || store.change(Workbench::cancel_update),
        move || update_window(store, system_dark),
    );
    if !opened {
        store.report("无法打开软件更新窗口");
    }
}

fn update_window_attributes(store: Store) -> nsh_core::desktop::WindowAttributes {
    use crate::workbench::UpdateState;
    let available = store.read(|s| {
        matches!(
            s.update_state,
            UpdateState::Available | UpdateState::Downloaded
        )
    });
    let mut attributes = nsh_core::desktop::WindowAttributes::default();
    let mut titlebar = nsh_core::desktop::TitleBarOptions::default();
    titlebar.style = nsh_core::desktop::TitleBarStyle::Hidden;
    titlebar.height = Some(px(32.));
    attributes.title_bar = Some(titlebar);
    attributes.closable = Some(available);
    attributes.minimizable = Some(available);
    attributes.maximizable = Some(available);
    attributes.resizable = Some(available);
    attributes.min_size = Some(Some(Size {
        width: px(if available { 532. } else { 400. }),
        height: px(if available { 402. } else { 146. }),
    }));
    attributes
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy)]
struct UpdateTheme(Memo<std::sync::Arc<nsh_base::Theme>>);

fn install_update_theme(initial_dark: bool, large: Memo<bool>) {
    #[cfg(target_os = "macos")]
    {
        let system_dark = system_update_appearance::watch(initial_dark);
        let resolved = computed(move || {
            let original = theme();
            let mut resolved = (*original).clone();
            for (name, color) in original.tokens().colors() {
                let selected = if system_dark.get() {
                    color.dark
                } else {
                    color.light
                };
                resolved.set_color(name, selected.into());
            }
            std::sync::Arc::new(resolved)
        });
        provide_context(UpdateTheme(resolved));
        let window = nsh_core::element::desktop::current_window();
        let key = computed(move || {
            (
                dark(),
                window.state().map(|s| (s.visible, s.focused)),
                large.get(),
            )
        });
        let positioned = std::sync::Arc::new(std::sync::Mutex::new(None::<bool>));
        effect(move || {
            let (_, _, available) = key.get();
            let positioned = positioned.clone();
            // Only this native update window follows the OS. The main app keeps its preference.
            drop(nsh_core::element::run_on_main_thread(move || {
                system_update_appearance::reset_window(available, &positioned);
            }));
        });
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (initial_dark, large);
}

fn update_theme() -> std::sync::Arc<nsh_base::Theme> {
    #[cfg(target_os = "macos")]
    if let Some(scoped) = use_context::<UpdateTheme>() {
        return scoped.0.get();
    }
    theme()
}

/// Content a software-update window presents while the host owns its operation.
#[derive(Debug, Clone, Default, PartialEq, JsObject)]
pub struct UpdatePresentation {
    /// Checking, downloading, installing or available.
    pub phase: UpdatePhase,
    /// Version offered by the verified update feed.
    pub version: String,
    /// Installed ChipDock version.
    pub current_version: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PropEnum)]
pub enum UpdatePhase {
    #[default]
    Checking,
    Downloading,
    Installing,
    Available,
}

#[derive(Clone, Copy)]
struct UpdateControls {
    store: Option<Store>,
    notes: Memo<String>,
}

fn render_update(
    renderer: Prop<Option<Renderer<UpdatePresentation>>>,
    presentation: impl Fn() -> UpdatePresentation + 'static,
) -> View {
    let presentation = computed(presentation);
    view()
        .flex_1()
        .min_h_0()
        .bg(move || update_theme().background)
        .text_color(move || update_theme().foreground)
        .text_size(body_size)
        .line_height(nsh_core::LineHeight::Multiple(1.2))
        .child(dynamic(move || {
            renderer
                .get()
                .expect("App.tsx supplies the update window")
                .render("software-update", presentation.get())
                .flex_1()
                .min_h_0()
        }))
}

fn update_window(store: Store, system_dark: bool) -> View {
    use crate::workbench::UpdateState;
    crate::ui::commands::watch_window(store, "update", None);
    let state = computed(move || store.read(|s| s.update_state.clone()));
    let large = computed(move || {
        matches!(
            state.get(),
            UpdateState::Available | UpdateState::Downloaded
        )
    });
    install_update_theme(system_dark, large);
    let window = nsh_core::element::desktop::current_window();
    let bounds_window = window.clone();
    let previous_size = stored(None::<bool>);
    effect(move || {
        let available = large.get();
        let previous = previous_size.get();
        previous_size.replace(Some(available));
        if previous == Some(available) || (previous.is_none() && !available) {
            return;
        }
        let frame = nsh_core::desktop::WindowFrame {
            width: Some(px(if available { 532. } else { 400. })),
            height: Some(px(if available { 402. } else { 146. })),
            ..Default::default()
        };
        bounds_window.set_bounds(frame);
    });
    let alerted = stored(None::<UpdateState>);
    let alert_window = window.clone();
    effect(move || {
        let next = state.get();
        let presentation = match &next {
            UpdateState::Current => Some((
                "您使用的就是最新版！",
                format!("ChipDock {}是当前的最新版本。", env!("CARGO_PKG_VERSION")),
                "好",
            )),
            UpdateState::Failed(message) => Some(("更新错误！", message.clone(), "取消更新")),
            UpdateState::Cancelled => {
                alert_window.close();
                None
            }
            _ => {
                alerted.replace(None);
                None
            }
        };
        if let Some((title, message, action)) = presentation {
            if alerted.get().as_ref() == Some(&next) {
                return;
            }
            alerted.replace(Some(next));
            let window = alert_window.clone();
            // Keep the checking window owner alive until the native result is answered.
            spawn(async move {
                if let Err(error) = modal_update_alert(title, message, action).await {
                    store.report(error);
                }
                window.close();
            });
        }
    });
    provide_context(UpdateControls {
        store: Some(store),
        notes: computed(move || {
            store.read(|s| {
                s.update
                    .as_ref()
                    .map(|u| u.notes.clone())
                    .unwrap_or_default()
            })
        }),
    });
    render_update(store.update_view, move || UpdatePresentation {
        phase: match state.get() {
            UpdateState::Available | UpdateState::Downloaded => UpdatePhase::Available,
            UpdateState::Downloading { .. } => UpdatePhase::Downloading,
            UpdateState::Installing => UpdatePhase::Installing,
            _ => UpdatePhase::Checking,
        },
        version: store.read(|s| {
            s.update
                .as_ref()
                .map(|u| u.version.clone())
                .unwrap_or_default()
        }),
        current_version: env!("CARGO_PKG_VERSION").into(),
    })
}

fn update_progress_bar(state: Memo<crate::workbench::UpdateState>) -> impl IntoElement {
    use crate::workbench::UpdateState;
    let track_width = signal(0_f32);
    let offset = animated_value(0.);
    effect(move || {
        let checking = matches!(state.get(), UpdateState::Checking);
        let width = track_width.get();
        if checking && !reduce_motion() && width > 0. {
            offset.set(-width * 0.66);
            let animation = Animation::timing(
                &offset,
                width,
                Timing::new(
                    update_theme().number("progress-loop").unwrap_or(1200.),
                    Easing::Linear,
                ),
            )
            .looped(None);
            drop(animation.start());
            cleanup(move || animation.stop());
        } else {
            offset.stop();
            offset.set(0.);
        }
    });
    nsh_base::progress()
        .value(move || match state.get() {
            UpdateState::Downloading { done, total } if total > 0 => {
                done as f32 / total as f32 * 100.
            }
            _ => 0.,
        })
        .indeterminate(move || !matches!(state.get(),UpdateState::Downloading{total,..}if total>0))
        .accessibility_label("软件更新进度")
        .track_style(|s| s.hidden())
        .child(
            view()
                .h(px(8.))
                .rounded_full()
                .overflow_hidden()
                .bg(move || update_theme().foreground.opacity(0.08))
                .on_layout(move |bounds: &Bounds<Points>| track_width.set(bounds.size.width.0))
                .child(
                    view()
                        .h(px(8.))
                        .rounded_full()
                        .bg(move || update_theme().primary)
                        .w(move || {
                            relative(match state.get() {
                                UpdateState::Downloading { done, total } if total > 0 => {
                                    (done as f32 / total as f32).clamp(0., 1.)
                                }
                                _ => 0.66,
                            })
                        })
                        .bind_translate_x(&offset),
                ),
        )
}

fn skip_update(store: Store, window: &nsh_core::element::desktop::WindowRef) {
    let result = store.change(|s| {
        let Some(version) = s.update.as_ref().map(|u| u.version.clone()) else {
            return Ok(());
        };
        let previous = s
            .preferences
            .fields
            .insert("skipped_update_version".into(), version);
        if let Err(error) = s.save() {
            match previous {
                Some(value) => {
                    s.preferences
                        .fields
                        .insert("skipped_update_version".into(), value);
                }
                None => {
                    s.preferences.fields.remove("skipped_update_version");
                }
            }
            return Err(error);
        }
        Ok(())
    });
    match result {
        Ok(()) => window.close(),
        Err(error) => {
            let message = format!("无法保存跳过的版本：{error:#}");
            store.report(&message);
            spawn(async move {
                let _ = modal_update_alert("无法保存跳过的版本", message, "好").await;
            });
        }
    }
}

fn install_update(store: Store) {
    store.change(|s| {
        if s.downloaded {
            s.request_install();
        } else if let Some(update) = s.update.clone() {
            s.submit(Command::DownloadUpdate(update));
        }
    });
}

#[derive(Debug, Clone, PartialEq, Props)]
pub struct UpdateTitleProps {
    /// Title shown using the update window's system appearance.
    pub children: Prop<Children>,
}

/// Applies the system appearance to the update title bar independently of the application theme.
#[component]
pub fn UpdateTitle(props: UpdateTitleProps) -> impl IntoElement {
    view()
        .window_drag(true)
        .border_b_1()
        .border_color(move || update_theme().foreground.opacity(0.1))
        .bg(move || update_theme().foreground.opacity(0.035))
        .text_color(move || update_theme().muted_foreground)
        .child(props.children)
}

/// Application artwork sized for the update window's platform.
#[component]
pub fn UpdateIcon() -> impl IntoElement {
    image(app_icon()).size(app_icon_extent(false))
}

/// An accessible native progress indicator with reduced-motion animation policy.
#[component]
pub fn DownloadProgress() -> impl IntoElement {
    let store = update_controls()
        .store
        .expect("progress belongs to an active update");
    update_progress_bar(computed(move || store.read(|s| s.update_state.clone())))
}

/// Selectable, themed Markdown from the verified release feed.
#[component]
pub fn ReleaseNotes() -> impl IntoElement {
    let notes = update_controls().notes;
    use nsh_base::{TextViewPart, TextViewParts};
    provide_context(
        TextViewParts::new()
            .part(TextViewPart::Root, |s| {
                s.text_size(px(13.))
                    .line_height(px(16.))
                    .gap(px(21.5))
                    .text_color(update_notes_ink)
            })
            .part(TextViewPart::Heading1, |s| {
                s.text_size(px(19.5))
                    .line_height(px(23.4))
                    .font_weight(FontWeight::BOLD)
                    .letter_spacing(px(0.))
            })
            .part(TextViewPart::Heading2, |s| {
                s.text_size(px(16.9))
                    .line_height(px(20.28))
                    .font_weight(FontWeight::BOLD)
                    .letter_spacing(px(0.))
                    .mt(px(-1.3))
            })
            .part(TextViewPart::List, |s| s.mt(px(-9.)).gap(px(3.75)))
            .part(TextViewPart::ListItem, |s| s.ps(px(36.)))
            .part(TextViewPart::ListItemContent, |s| s.line_height(px(16.)))
            .part(TextViewPart::ListMarker, |s| {
                s.text_color(update_notes_ink)
                    .text_size(px(13.))
                    .font_family("Menlo")
                    .pe(px(10.))
            }),
    );
    scroll()
        .flex_1()
        .min_h_0()
        .rounded(px(5.))
        .border_1()
        .border_color(move || update_theme().foreground.opacity(0.13))
        .child(
            nsh_base::text_view()
                .markdown(move || Some(notes.get().into()))
                .selectable(true)
                .text_color(update_notes_ink)
                .px(px(9.))
                .py(px(8.)),
        )
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PropEnum)]
pub enum UpdateCommand {
    #[default]
    Cancel,
    Skip,
    Install,
}

#[derive(Debug, Clone, PartialEq, Props)]
pub struct UpdateButtonProps {
    /// The guarded workbench command.
    pub command: Prop<UpdateCommand>,
    /// Visible and accessible title.
    pub label: Prop<SharedString>,
}

/// Update button with native window appearance and installation guards.
#[component]
pub fn UpdateButton(props: UpdateButtonProps) -> impl IntoElement {
    let live = update_controls().store;
    let command = props.command.get();
    let window = nsh_core::element::desktop::current_window();
    update_action(
        props.label,
        command == UpdateCommand::Install,
        move || {
            live.is_none_or(|store| match command {
                UpdateCommand::Cancel => store
                    .read(|s| matches!(s.update_state, crate::workbench::UpdateState::Installing)),
                UpdateCommand::Skip => false,
                UpdateCommand::Install => store.read(|s| {
                    !s.update.as_ref().is_some_and(|u| u.can_install)
                        || s.lane_busy(Lane::Update)
                        || !s.can_install_update()
                }),
            })
        },
        move || {
            if let Some(store) = live {
                match command {
                    UpdateCommand::Cancel => {
                        store.change(Workbench::cancel_update);
                        window.close();
                    }
                    UpdateCommand::Skip => skip_update(store, &window),
                    UpdateCommand::Install => install_update(store),
                }
            }
        },
    )
}

fn update_controls() -> UpdateControls {
    use_context().expect("update control belongs to a software-update window")
}
fn update_notes_ink() -> nsh_core::element::AdaptiveColor {
    update_theme()
        .color("update-notes-foreground")
        .unwrap_or(update_theme().foreground)
}

/// Renderer inspection only: no Workbench, transport, update selection, or enabled actions.
#[cfg(debug_assertions)]
pub(crate) fn preview_available(
    feed: crate::update::Feed,
    renderer: Prop<Option<Renderer<UpdatePresentation>>>,
) -> View {
    if let Ok(tokens) = nsh_base::theme_overrides(include_str!("../../theme.json")) {
        set_tokens(tokens);
    }
    if let Some(path) = std::env::var_os("CHIPDOCK_PREFERENCES_PATH") {
        let path = std::path::PathBuf::from(path);
        if path.is_absolute()
            && let Ok(preferences) = crate::model::Preferences::load(&path)
        {
            super::set_appearance(&preferences.theme);
        }
    }
    let version = feed.version;
    provide_context(UpdateControls {
        store: None,
        notes: computed(move || feed.notes.clone()),
    });
    let window = nsh_core::element::desktop::current_window();
    let mut attributes = nsh_core::desktop::WindowAttributes::default();
    let mut titlebar = nsh_core::desktop::TitleBarOptions::default();
    titlebar.style = nsh_core::desktop::TitleBarStyle::Hidden;
    titlebar.height = Some(px(32.));
    attributes.title = Some("软件更新".into());
    attributes.title_bar = Some(titlebar);
    attributes.min_size = Some(Some(Size {
        width: px(532.),
        height: px(402.),
    }));
    attributes.max_size = Some(None);
    attributes.resizable = Some(true);
    attributes.minimizable = Some(true);
    attributes.maximizable = Some(true);
    attributes.closable = Some(true);
    attributes.close_on_press = Some(true);
    window.set(attributes);
    window.set_bounds(nsh_core::desktop::WindowFrame {
        width: Some(px(532.)),
        height: Some(px(402.)),
        ..Default::default()
    });
    window.center(None);
    render_update(renderer, move || UpdatePresentation {
        phase: UpdatePhase::Available,
        version: version.clone(),
        current_version: env!("CARGO_PKG_VERSION").into(),
    })
}

fn update_action(
    label: Prop<SharedString>,
    primary: bool,
    disabled: impl Fn() -> bool + 'static,
    press: impl Fn() + 'static,
) -> impl IntoElement {
    nsh_base::button()
        .on_press(press)
        .disabled(disabled)
        .accessibility_label(move || Some(label.get()))
        .min_w(px(100.))
        .h(px(28.))
        .px(px(14.))
        .rounded_full()
        .items_center()
        .justify_center()
        .bg(move || {
            if primary {
                update_theme().primary
            } else {
                update_theme().foreground.opacity(0.08)
            }
        })
        .text_color(move || {
            if primary {
                update_theme().primary_foreground
            } else {
                update_theme().foreground
            }
        })
        .hover(move |s| {
            s.bg(move || {
                if primary {
                    update_theme().primary.opacity(0.9)
                } else {
                    update_theme().accent
                }
            })
        })
        .active(move |s| s.opacity(0.8))
        .focus_visible(|s| s.outline(px(2.)).outline_color(move || update_theme().ring))
        .disabled_style(|s| s.opacity(0.4))
        .child(
            text(label)
                .text_size(body_size)
                .font_weight(FontWeight::MEDIUM),
        )
}

async fn modal_update_alert(
    title: &'static str,
    message: String,
    action: &'static str,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let (answer, result) = tokio::sync::oneshot::channel();
        // AppKit's app-modal loop must start on a later run-loop turn, outside
        // NSH's main-queue drain. NSH's native alert scheduler uses the same modes.
        nsh_core::element::run_on_main_thread(move || -> Result<(), String> {
            use block2::RcBlock;
            use nsh_platform_macos::{
                objc2::MainThreadMarker,
                objc2_app_kit::{NSAlert, NSAlertStyle, NSApplication, NSModalPanelRunLoopMode},
                objc2_foundation::{NSArray, NSDefaultRunLoopMode, NSRunLoop, NSString},
            };
            let marker =
                MainThreadMarker::new().ok_or_else(|| "无法在主线程显示更新结果".to_owned())?;
            let app = NSApplication::sharedApplication(marker);
            let alert = NSAlert::new(marker);
            alert.setAlertStyle(NSAlertStyle::Informational);
            alert.setMessageText(&NSString::from_str(title));
            alert.setInformativeText(&NSString::from_str(&message));
            alert.addButtonWithTitle(&NSString::from_str(action));
            let answer = std::cell::RefCell::new(Some(answer));
            let block = RcBlock::new(move || {
                for window in app.windows().iter() {
                    if window.title().to_string() == "软件更新" {
                        window.orderOut(None);
                    }
                }
                app.activate();
                let response = alert.runModal();
                alert.window().orderOut(None);
                if let Some(answer) = answer.borrow_mut().take() {
                    let outcome = if response >= 1000 {
                        Ok(())
                    } else {
                        Err(format!("更新结果窗口未能显示（{response}）"))
                    };
                    let _ = answer.send(outcome);
                }
            });
            // SAFETY: Foundation/AppKit constants are immutable. The copied
            // block executes on this same main thread in either named mode.
            let (default, modal) = unsafe { (NSDefaultRunLoopMode, NSModalPanelRunLoopMode) };
            let modes = NSArray::from_slice(&[default, modal]);
            // SAFETY: the run loop retains the block and invokes it only on the main thread.
            unsafe { NSRunLoop::mainRunLoop().performInModes_block(&modes, &block) };
            Ok(())
        })
        .await
        .map_err(|error| error.to_string())??;
        result.await.map_err(|error| error.to_string())?
    }
    #[cfg(not(target_os = "macos"))]
    {
        nsh_base::alert_dialog()
            .title(title)
            .description(message)
            .action("ok", action)
            .show()
            .await
            .map(|_| ())
    }
}

#[cfg(target_os = "macos")]
mod system_update_appearance {
    use super::*;
    use nsh_platform_macos::{
        objc2::{
            MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained,
            runtime::AnyObject,
        },
        objc2_app_kit::{NSAppearanceCustomization, NSApplication},
        objc2_foundation::{
            NSKeyValueObservingOptions, NSObject, NSObjectNSKeyValueObserverRegistration,
            NSObjectProtocol, NSString,
        },
    };
    use std::{
        cell::{Cell, RefCell},
        collections::BTreeMap,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
    };
    use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

    define_class!(
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "ChipDockUpdateAppearanceObserver"]
        struct Observer;

        unsafe impl NSObjectProtocol for Observer {}

        impl Observer {
            #[unsafe(method(observeValueForKeyPath:ofObject:change:context:))]
            fn changed(
                &self,
                _key: Option<&NSString>,
                _object: Option<&AnyObject>,
                _change: Option<&AnyObject>,
                _context: *mut std::ffi::c_void,
            ) {
                let value = system_dark(MainThreadMarker::from(self));
                WATCHES.with_borrow(|watches| {
                    for watch in watches.values() {
                        let _ = watch.sender.send(value);
                    }
                });
            }
        }
    );

    struct Watch {
        observer: Retained<Observer>,
        sender: UnboundedSender<bool>,
    }
    thread_local! {
        static WATCHES: RefCell<BTreeMap<u64, Watch>> = const { RefCell::new(BTreeMap::new()) };
        static NEXT: Cell<u64> = const { Cell::new(1) };
    }

    fn system_dark(marker: MainThreadMarker) -> bool {
        NSApplication::sharedApplication(marker)
            .effectiveAppearance()
            .name()
            .to_string()
            .contains("Dark")
    }

    pub(super) async fn initial() -> Result<bool, String> {
        nsh_core::element::run_on_main_thread(|| {
            MainThreadMarker::new()
                .map(system_dark)
                .ok_or_else(|| "无法读取系统更新窗口外观".to_owned())
        })
        .await
        .map_err(|error| error.to_string())?
    }

    pub(super) fn watch(initial: bool) -> Signal<bool> {
        let value = signal(initial);
        let (sender, mut receiver) = unbounded_channel();
        let registration = Arc::new(Mutex::new(None::<u64>));
        let cancelled = Arc::new(AtomicBool::new(false));
        let close_registration = registration.clone();
        let close_cancelled = cancelled.clone();
        cleanup(move || {
            close_cancelled.store(true, Ordering::Release);
            drop(nsh_core::element::run_on_main_thread(move || {
                if let Some(id) = close_registration.lock().unwrap().take() {
                    remove(id);
                }
            }));
        });
        spawn(async move {
            let _ = nsh_core::element::run_on_main_thread(move || {
                if cancelled.load(Ordering::Acquire) {
                    return;
                }
                let Some(marker) = MainThreadMarker::new() else {
                    return;
                };
                let app = NSApplication::sharedApplication(marker);
                // SAFETY: NSObject's initializer, on the main thread.
                let observer: Retained<Observer> =
                    unsafe { msg_send![Observer::alloc(marker), init] };
                let id = NEXT.with(|next| {
                    let id = next.get();
                    next.set(id + 1);
                    id
                });
                // SAFETY: effectiveAppearance is the public KVO appearance property.
                // The observer stays retained in WATCHES until this window's owner ends.
                unsafe {
                    app.addObserver_forKeyPath_options_context(
                        &observer,
                        &NSString::from_str("effectiveAppearance"),
                        NSKeyValueObservingOptions::New,
                        std::ptr::null_mut(),
                    );
                }
                let _ = sender.send(system_dark(marker));
                WATCHES.with_borrow_mut(|watches| {
                    watches.insert(id, Watch { observer, sender });
                });
                *registration.lock().unwrap() = Some(id);
            })
            .await;
            while let Some(next) = receiver.recv().await {
                value.set(next);
            }
        });
        value
    }

    fn remove(id: u64) {
        let watch = WATCHES.with_borrow_mut(|watches| watches.remove(&id));
        if let Some(watch) = watch {
            let app = NSApplication::sharedApplication(MainThreadMarker::from(&*watch.observer));
            // SAFETY: this is the exact observer/key registered by watch(); both live here.
            unsafe {
                app.removeObserver_forKeyPath(
                    &watch.observer,
                    &NSString::from_str("effectiveAppearance"),
                );
            }
        }
    }

    pub(super) fn reset_window(available: bool, positioned: &Mutex<Option<bool>>) {
        let Some(marker) = MainThreadMarker::new() else {
            return;
        };
        for window in NSApplication::sharedApplication(marker).windows().iter() {
            if window.title().to_string() == "软件更新" {
                window.setAppearance(None);
                if let Some(content) = window.contentView() {
                    content.setAppearance(None);
                    for view in content.subviews().iter() {
                        view.setAppearance(None);
                    }
                }
                let center = {
                    let mut last = positioned.lock().unwrap();
                    if *last == Some(available) {
                        false
                    } else {
                        *last = Some(available);
                        true
                    }
                };
                if center {
                    window.center();
                    if !available {
                        // The measured native status-window origin is two points
                        // below AppKit's center for its final frame.
                        let mut origin = window.frame().origin;
                        origin.y -= 2.;
                        window.setFrameOrigin(origin);
                    }
                }
            }
        }
    }
}

/// Theme picker with the platform's native menu and appearance policy.
#[component]
pub fn ThemePicker() -> impl IntoElement {
    appearance_menu(settings_store())
}

#[derive(Debug, Clone, PartialEq, Props)]
pub struct FlashDefaultProps {
    /// True for write verification, false for reset after writing.
    pub verify: Prop<bool>,
}

/// A persisted default for newly created flashing sessions.
#[component]
pub fn FlashDefault(props: FlashDefaultProps) -> impl IntoElement {
    let verify = props.verify.get();
    let (key, label) = if verify {
        ("default_verify", "写入后校验")
    } else {
        ("default_reset", "完成后复位")
    };
    preference_switch(settings_store(), key, label, verify)
}

#[derive(Debug, Clone, PartialEq, Props)]
pub struct AutomaticUpdateProps {
    /// Controls download and installation together.
    pub combined: Prop<bool>,
}

/// Persisted automatic update preference.
#[component]
pub fn AutomaticUpdate(props: AutomaticUpdateProps) -> impl IntoElement {
    automatic_update_toggle(settings_store(), props.combined.get())
}

/// Platform-specific application artwork at its measured About size.
#[component]
pub fn AboutIcon() -> impl IntoElement {
    image(app_icon())
        .size(app_icon_extent(true))
        .label("ChipDock 应用图标")
}

/// Opens the update window and starts a guarded manual check.
#[component]
pub fn CheckForUpdates() -> impl IntoElement {
    let store = settings_store();
    action(
        "检查更新…",
        Variant::Default,
        move || store.read(|s| s.lane_busy(Lane::Update)),
        move || manual_check(store),
    )
}

fn settings_store() -> Store {
    use_context().expect("settings window owns the workbench context")
}

pub(super) fn register(registry: &mut Registry) {
    registry
        .register::<ThemePicker>()
        .register::<FlashDefault>()
        .register::<AutomaticUpdate>()
        .register::<AboutIcon>()
        .register::<UpdateTitle>()
        .register::<UpdateIcon>()
        .register::<DownloadProgress>()
        .register::<ReleaseNotes>()
        .register::<UpdateButton>()
        .register::<CheckForUpdates>();
}
