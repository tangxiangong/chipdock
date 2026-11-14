//! Auxiliary windows keep the native client's individual geometry and content.
use super::*;
use nsh_core::element::desktop::{open_window, window};

#[path = "logs.rs"]
mod logs;
#[path = "memory.rs"]
mod memory;
#[path = "plot.rs"]
mod plot;
#[path = "settings.rs"]
mod settings;

#[derive(Clone, Copy)]
pub(super) enum Panel {
    Memory,
    Logs,
    Settings,
    Profiles,
    Plot(u64),
}

#[cfg(debug_assertions)]
pub(super) fn preview_available(feed: crate::update::Feed) -> View {
    settings::preview_available(feed)
}

pub(super) fn erase_memory(store: Store) {
    memory::erase(store);
}

pub(super) fn check_update(store: Store) {
    settings::manual_check(store);
}

pub(super) fn open(store: Store, panel: Panel) {
    if matches!(panel, Panel::Profiles) {
        tools::open(store, "profile");
        return;
    }
    if let Panel::Plot(id) = panel {
        let exists = store.change(|s| {
            if let Some(session) = s.sessions.get_mut(&id) {
                session.plotting = true;
                true
            } else {
                false
            }
        });
        if !exists {
            store.report("会话已关闭");
            return;
        }
    }
    let (id, title, width, height, minimum_width, minimum_height) = match panel {
        // Native titlebar windows add 32 pt above these content sizes, as in the tuned GPUI client.
        Panel::Memory => ("memory".into(), "目标内存", 560., 568., 420., 328.),
        Panel::Logs => ("logs".into(), "活动日志", 720., 440., 500., 240.),
        Panel::Settings => ("settings".into(), "设置", 480., 418., 480., 418.),
        Panel::Plot(id) => (format!("plot-{id}"), "实时曲线", 820., 488., 720., 388.),
        Panel::Profiles => return,
    };
    let mut attributes = nsh_core::desktop::WindowAttributes::default();
    attributes.min_size = Some(Some(Size {
        width: px(minimum_width),
        height: px(minimum_height),
    }));
    if matches!(panel, Panel::Logs | Panel::Settings) {
        let mut titlebar = nsh_core::desktop::TitleBarOptions::default();
        titlebar.style = nsh_core::desktop::TitleBarStyle::Hidden;
        titlebar.height = Some(px(if matches!(panel, Panel::Logs) {
            52.
        } else {
            32.
        }));
        if matches!(panel, Panel::Logs) {
            titlebar.controls_offset = Some(Point {
                x: px(19.),
                y: px(19.),
            });
        }
        attributes.title_bar = Some(titlebar);
    }
    if matches!(panel, Panel::Settings) {
        attributes.resizable = Some(false);
        attributes.minimizable = Some(false);
        attributes.maximizable = Some(false);
        attributes.max_size = attributes.min_size;
    }
    let closed_id = id.clone();
    let opened = open_window(
        window()
            .id(id.clone())
            .title(title.to_owned())
            .parent("main")
            .default_position(nsh_core::desktop::WindowPosition::Center)
            .restore_bounds(!matches!(panel, Panel::Settings))
            .attributes(attributes)
            .default_size(Size {
                width: px(width),
                height: px(height),
            })
            .on_open_change(move |event| {
                if !event.open {
                    store.windows.with(|windows| {
                        windows.remove(&closed_id);
                    });
                    store.change(|s| {
                        if matches!(panel, Panel::Logs) {
                            s.set_logs_visible(false);
                        }
                        if let Panel::Plot(id) = panel
                            && let Some(session) = s.sessions.get_mut(&id)
                        {
                            session.plotting = false;
                            session.plot_revision += 1;
                        }
                    });
                }
            })
            .content(move || {
                match panel {
                    Panel::Settings => crate::ui::commands::watch_window(store, "settings", None),
                    Panel::Plot(_) => crate::ui::commands::watch_window(store, "plot", None),
                    _ => {}
                }
                view()
                    .flex_1()
                    .min_h_0()
                    .bg(move || theme().background)
                    .text_color(move || theme().foreground)
                    .text_size(body_size)
                    .line_height(nsh_core::LineHeight::Multiple(1.2))
                    .child(match panel {
                        Panel::Memory => memory::page(store).into_element(),
                        Panel::Logs => logs::page(store).into_element(),
                        Panel::Settings => settings::page(store).into_element(),
                        Panel::Plot(id) => plot::page(store, id).into_element(),
                        Panel::Profiles => view().into_element(),
                    })
            }),
    );
    if let Some(window) = opened {
        store.windows.with(|windows| {
            windows.insert(id, window);
        });
        store.change(|s| {
            if matches!(panel, Panel::Logs) {
                s.set_logs_visible(true);
            }
        });
    } else {
        store.report(format!("无法打开{title}窗口"));
    }
}

fn empty_view(
    symbol: impl Fn() -> &'static str + 'static,
    title: impl IntoReactive<SharedString>,
    description: impl IntoReactive<SharedString>,
) -> View {
    empty_view_with_layout(symbol, title, description, || EmptyLayout::Standard)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EmptyLayout {
    Standard,
    Search,
    LabelOnly,
}

fn empty_view_with_layout(
    symbol: impl Fn() -> &'static str + 'static,
    title: impl IntoReactive<SharedString>,
    description: impl IntoReactive<SharedString>,
    layout: impl Fn() -> EmptyLayout + 'static,
) -> View {
    let layout = computed(layout);
    let description = std::rc::Rc::new(description.into_reactive());
    let visible = description.clone();
    // Native ContentUnavailableView balances its artwork above the geometric centre.
    view()
        .flex_1()
        .min_h_0()
        .pb(move || {
            px(match layout.get() {
                EmptyLayout::Standard => 17.,
                EmptyLayout::Search => 15.,
                EmptyLayout::LabelOnly => 3.,
            })
        })
        .items_center()
        .justify_center()
        .gap(px(6.))
        .text_color(move || {
            theme()
                .color("empty-foreground")
                .unwrap_or(theme().muted_foreground)
        })
        .child(dynamic(move || {
            icons::icon(symbol())
                .size(move || {
                    px(match layout.get() {
                        EmptyLayout::Standard => 44.,
                        EmptyLayout::Search => 46.,
                        EmptyLayout::LabelOnly => 47.,
                    })
                })
                .mb(px(10.))
                .text_color(move || {
                    theme()
                        .color("empty-symbol")
                        .unwrap_or(theme().muted_foreground.opacity(0.5))
                })
        }))
        .child(text(title).text_size(px(15.)).font_weight(FontWeight::BOLD))
        .child(when(
            move || !visible.get().is_empty(),
            move || {
                text({
                    let description = description.clone();
                    move || description.get()
                })
                .text_size(px(15.))
            },
        ))
}
