//! App-owned NSH control styling. Feature pages never style base controls.
use super::*;
use nsh_base::{SelectEntry, SelectItem, button, checkbox, input, select};

pub(super) const INSTANT: f32 = 120.;
pub(super) const FAST: f32 = 180.;
pub(super) const BASE: f32 = 240.;
#[derive(Clone, Copy)]
pub(super) enum Variant {
    Default,
    Primary,
    DefaultAction,
    Ghost,
}

#[derive(Clone, Copy)]
pub(super) struct MainWindowModal(pub Signal<bool>, pub Signal<bool>);
#[derive(Clone, Copy)]
pub(super) struct ModalControls;
#[derive(Clone, Copy)]
pub(super) struct NativeMenuActivity(pub Signal<usize>);

pub(super) fn native_menu_tracking() -> Memo<bool> {
    let activity = use_context::<NativeMenuActivity>();
    computed(move || activity.is_some_and(|activity| activity.0.get() > 0))
}

struct NativeMenuTrackingGuard {
    activity: Option<NativeMenuActivity>,
    open: Signal<bool>,
}

impl NativeMenuTrackingGuard {
    fn new(activity: Option<NativeMenuActivity>, open: Signal<bool>) -> Self {
        if let Some(activity) = activity
            && let Some(count) = activity.0.try_get()
        {
            activity.0.try_set(count + 1);
        }
        open.try_set(true);
        Self { activity, open }
    }
}

impl Drop for NativeMenuTrackingGuard {
    fn drop(&mut self) {
        self.open.try_set(false);
        if let Some(activity) = self.activity
            && let Some(count) = activity.0.try_get()
        {
            activity.0.try_set(count.saturating_sub(1));
        }
    }
}

pub(super) fn control_active() -> Memo<bool> {
    let window = nsh_core::element::desktop::current_window();
    let modal = use_context::<MainWindowModal>();
    let modal_controls = use_context::<ModalControls>().is_some();
    computed(move || {
        window.state().is_none_or(|state| state.focused)
            && (window.id() != "main"
                || modal.is_none_or(|modal| !modal.1.get() && (modal_controls || !modal.0.get())))
    })
}

pub(super) fn default_push_fill(disabled: bool) -> AdaptiveColor {
    if disabled {
        theme().color("default-push-disabled").unwrap_or(
            theme()
                .color("button-disabled")
                .unwrap_or(theme().secondary),
        )
    } else {
        theme().primary
    }
}
fn default_push_ink(disabled: bool) -> AdaptiveColor {
    if disabled {
        theme()
            .color("default-push-disabled-foreground")
            .unwrap_or(theme().muted_foreground)
    } else {
        theme().primary_foreground
    }
}
pub(super) fn emphasized_fill(
    primary: bool,
    disabled: bool,
    active: bool,
    default_action: bool,
) -> AdaptiveColor {
    if default_action && disabled {
        default_push_fill(true)
    } else if !active && primary {
        theme()
            .color(if disabled {
                "primary-inactive-disabled"
            } else {
                "primary-inactive"
            })
            .unwrap_or(
                theme()
                    .color("selection-inactive")
                    .unwrap_or(theme().secondary),
            )
    } else if default_action {
        default_push_fill(disabled)
    } else {
        button_fill(primary, disabled)
    }
}
pub(super) fn emphasized_ink(
    primary: bool,
    disabled: bool,
    active: bool,
    default_action: bool,
) -> AdaptiveColor {
    if default_action && disabled {
        default_push_ink(true)
    } else if !active {
        theme()
            .color(if disabled {
                "control-inactive-disabled-foreground"
            } else {
                "control-inactive-foreground"
            })
            .unwrap_or(theme().muted_foreground)
    } else if default_action {
        default_push_ink(disabled)
    } else {
        button_ink(primary, disabled)
    }
}

pub(super) fn button_fill(primary: bool, disabled: bool) -> AdaptiveColor {
    theme()
        .color(match (primary, disabled) {
            (true, true) => "primary-disabled",
            (false, true) => "button-disabled",
            (false, false) => "button",
            (true, false) => "primary",
        })
        .unwrap_or(if primary {
            theme().primary
        } else {
            theme().secondary
        })
}

pub(super) fn button_ink(primary: bool, disabled: bool) -> AdaptiveColor {
    if disabled {
        theme()
            .color(if primary {
                "primary-disabled-foreground"
            } else {
                "button-disabled-foreground"
            })
            .unwrap_or(theme().muted_foreground)
    } else if primary {
        theme().primary_foreground
    } else {
        theme().foreground
    }
}

pub(super) fn action(
    label: impl IntoReactive<SharedString>,
    variant: Variant,
    disabled: impl Fn() -> bool + 'static,
    on_press: impl Fn() + 'static,
) -> impl IntoElement {
    styled_action(label, variant, disabled, on_press, false)
}

pub(super) fn form_action(
    label: impl IntoReactive<SharedString>,
    disabled: impl Fn() -> bool + 'static,
    on_press: impl Fn() + 'static,
) -> impl IntoElement {
    styled_action(label, Variant::Default, disabled, on_press, true)
}

fn styled_action(
    label: impl IntoReactive<SharedString>,
    variant: Variant,
    disabled: impl Fn() -> bool + 'static,
    on_press: impl Fn() + 'static,
    form: bool,
) -> impl IntoElement {
    let disabled = computed(disabled);
    let active = control_active();
    let modal_controls = use_context::<ModalControls>().is_some();
    button()
        .disabled(disabled)
        .on_press(on_press)
        .min_h(px(24.))
        .px_3()
        .py(px(2.))
        .rounded(move || theme().radius)
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .transition_colors()
        .duration(INSTANT)
        .bg(move || match variant {
            Variant::Primary => emphasized_fill(true, disabled.get(), active.get(), false),
            Variant::DefaultAction => emphasized_fill(true, disabled.get(), active.get(), true),
            Variant::Ghost => theme().background.opacity(0.),
            Variant::Default if form => theme()
                .color(if disabled.get() {
                    "form-action-disabled"
                } else {
                    "form-action"
                })
                .unwrap_or(theme().secondary),
            Variant::Default if modal_controls => {
                theme().color("dialog-button").unwrap_or(theme().secondary)
            }
            Variant::Default => theme().secondary,
        })
        .hover(move |s| {
            s.bg(move || match variant {
                Variant::Primary | Variant::DefaultAction => {
                    theme().color("primary-hover").unwrap_or(theme().primary)
                }
                _ => theme().accent,
            })
        })
        .active(move |s| {
            s.bg(move || match variant {
                Variant::Primary | Variant::DefaultAction => {
                    theme().color("primary-active").unwrap_or(theme().primary)
                }
                _ => theme().muted,
            })
        })
        .when(!reduce_motion(), |s| {
            s.transition_transform()
                .duration(FAST)
                .active(|s| s.scale(0.97))
        })
        .focus_visible(|s| s.outline(px(2.)).outline_color(move || theme().ring))
        .disabled_style(move |s| {
            s.opacity(
                if form || matches!(variant, Variant::Primary | Variant::DefaultAction) {
                    1.
                } else {
                    0.45
                },
            )
        })
        .child(
            text(label)
                .text_size(body_size)
                .text_color(move || match variant {
                    Variant::Primary => emphasized_ink(true, disabled.get(), active.get(), false),
                    Variant::DefaultAction => {
                        emphasized_ink(true, disabled.get(), active.get(), true)
                    }
                    _ if form => button_ink(false, disabled.get()),
                    _ => theme().foreground,
                }),
        )
}
pub(super) fn readonly(value: impl Fn() -> Option<SharedString> + 'static) -> impl IntoElement {
    nsh_base::editor()
        .value(value)
        .read_only(true)
        .line_numbers(false)
        .folding(false)
        .active_line(false)
        .indent_guides(false)
        .soft_wrap(false)
        .font_size(body_size())
        .line_height(px(body_size().0 * 1.2))
        .font_family(move || Some(mono()))
        .flex_1()
        .min_h_24()
}
pub(super) fn toggle(store: Store, key: &'static str, label: &'static str) -> impl IntoElement {
    let active = control_active();
    checkbox()
        .checked(move || store.flag(key))
        .on_change(move |v: &bool| store.change(|s| s.flag(key, *v)))
        .disabled(move || store.read(|s| s.installing))
        .accessibility_label(label)
        .flex_row()
        .items_center()
        .gap(px(6.))
        .min_h(px(16.))
        .focus_visible(|s| s.outline(px(2.)).outline_color(move || theme().ring))
        .hover(|s| s.bg(move || theme().accent))
        .active(|s| s.bg(move || theme().muted))
        .transition_colors()
        .duration(INSTANT)
        .indicator_style(move |s| {
            s.size_4()
                .rounded_sm()
                .border_1()
                .border_color(move || theme().color("checkbox").unwrap_or(theme().input))
                .items_center()
                .justify_center()
                .bg(move || theme().color("checkbox").unwrap_or(theme().background))
                .checked(|s| {
                    s.bg(move || {
                        if active.get() {
                            theme().primary
                        } else {
                            theme()
                                .color("selection-inactive")
                                .unwrap_or(theme().secondary)
                        }
                    })
                    .border_color(move || {
                        if active.get() {
                            theme().primary
                        } else {
                            theme()
                                .color("selection-inactive")
                                .unwrap_or(theme().secondary)
                        }
                    })
                })
        })
        .indicator(
            text(move || if store.flag(key) { "✓" } else { "" })
                .text_size(caption_size)
                .text_color(move || theme().primary_foreground),
        )
        .child(text(label).text_size(body_size))
}
pub(super) fn popup(s: nsh_core::element::StyleRefinement) -> nsh_core::element::StyleRefinement {
    let native = cfg!(target_os = "macos") && os_version().0 >= 26;
    let s = s.p_1().rounded(move || {
        if native {
            px(theme().number("menu-radius").unwrap_or(12.))
        } else {
            theme().radius
        }
    });
    if native {
        s.shadow_2xl().glass_effect(nsh_core::GlassEffect::Regular)
    } else {
        s.shadow_md()
            .border_1()
            .border_color(move || theme().border)
            .bg(move || theme().popover)
    }
}

pub(super) fn native_toggle(
    store: Store,
    key: &'static str,
    label: &'static str,
) -> nsh_core::element::desktop::MenuItem {
    nsh_core::element::desktop::MenuItem::new(label)
        .checked(store.flag(key))
        .on_select(move || store.change(|s| s.flag(key, !s.preferences.flag(key))))
}

/// The desktop's own menu supplies its keyboard navigation, focus, accessibility and material.
pub(super) fn native_menu(
    trigger: View,
    items: impl Fn() -> Vec<nsh_core::element::desktop::MenuItem> + 'static,
) -> impl IntoElement {
    native_menu_sized(
        trigger,
        || false,
        items,
        false,
        false,
        false,
        MenuPlacement::default(),
    )
}

pub(super) fn native_menu_disabled(
    trigger: View,
    disabled: impl Fn() -> bool + 'static,
    items: impl Fn() -> Vec<nsh_core::element::desktop::MenuItem> + 'static,
) -> impl IntoElement {
    native_menu_sized(
        trigger,
        disabled,
        items,
        false,
        false,
        false,
        MenuPlacement::default(),
    )
}

pub(super) fn native_menu_small_disabled(
    trigger: View,
    disabled: impl Fn() -> bool + 'static,
    items: impl Fn() -> Vec<nsh_core::element::desktop::MenuItem> + 'static,
) -> impl IntoElement {
    native_menu_sized(
        trigger,
        disabled,
        items,
        true,
        false,
        false,
        MenuPlacement::default(),
    )
}

pub(super) fn native_borderless_menu_disabled(
    trigger: View,
    disabled: impl Fn() -> bool + 'static,
    items: impl Fn() -> Vec<nsh_core::element::desktop::MenuItem> + 'static,
) -> impl IntoElement {
    native_menu_sized(
        trigger,
        disabled,
        items,
        false,
        true,
        false,
        MenuPlacement::default(),
    )
}

pub(super) fn native_borderless_menu_small(
    trigger: View,
    items: impl Fn() -> Vec<nsh_core::element::desktop::MenuItem> + 'static,
) -> impl IntoElement {
    native_menu_sized(
        trigger,
        || false,
        items,
        true,
        true,
        false,
        MenuPlacement::default(),
    )
}

pub(super) fn native_borderless_menu_small_offset(
    trigger: View,
    items: impl Fn() -> Vec<nsh_core::element::desktop::MenuItem> + 'static,
    below_offset: f32,
) -> impl IntoElement {
    native_menu_sized(
        trigger,
        || false,
        items,
        true,
        true,
        false,
        MenuPlacement {
            selected_row: None,
            below_offset,
            choice: None,
        },
    )
}
pub(super) fn native_borderless_menu_small_disabled(
    trigger: View,
    disabled: impl Fn() -> bool + 'static,
    items: impl Fn() -> Vec<nsh_core::element::desktop::MenuItem> + 'static,
) -> impl IntoElement {
    native_menu_sized(
        trigger,
        disabled,
        items,
        true,
        true,
        false,
        MenuPlacement::default(),
    )
}

pub(super) fn native_toolbar_menu(
    trigger: View,
    items: impl Fn() -> Vec<nsh_core::element::desktop::MenuItem> + 'static,
) -> impl IntoElement {
    native_menu_sized(
        trigger,
        || false,
        items,
        false,
        true,
        true,
        MenuPlacement::default(),
    )
}

pub(super) fn native_choice_menu(
    trigger: View,
    disabled: impl Fn() -> bool + 'static,
    selected_row: impl Fn() -> usize + 'static,
    small: bool,
    labels: impl Fn() -> Vec<String> + 'static,
    change: impl Fn(usize) + 'static,
) -> impl IntoElement {
    native_menu_sized(
        trigger,
        disabled,
        Vec::new,
        small,
        true,
        false,
        MenuPlacement {
            selected_row: Some(computed(selected_row)),
            below_offset: 0.,
            choice: Some(ChoicePopup {
                labels: std::rc::Rc::new(labels),
                change: std::rc::Rc::new(change),
            }),
        },
    )
}

pub(super) fn native_menu_small_offset_disabled(
    trigger: View,
    disabled: impl Fn() -> bool + 'static,
    items: impl Fn() -> Vec<nsh_core::element::desktop::MenuItem> + 'static,
    below_offset: f32,
) -> impl IntoElement {
    native_menu_sized(
        trigger,
        disabled,
        items,
        true,
        false,
        false,
        MenuPlacement {
            selected_row: None,
            below_offset,
            choice: None,
        },
    )
}

#[derive(Clone, Default)]
struct MenuPlacement {
    selected_row: Option<Memo<usize>>,
    below_offset: f32,
    choice: Option<ChoicePopup>,
}

#[derive(Clone)]
struct ChoicePopup {
    labels: std::rc::Rc<dyn Fn() -> Vec<String>>,
    change: std::rc::Rc<dyn Fn(usize)>,
}

fn native_menu_sized(
    trigger: View,
    disabled: impl Fn() -> bool + 'static,
    items: impl Fn() -> Vec<nsh_core::element::desktop::MenuItem> + 'static,
    small: bool,
    borderless: bool,
    toolbar_overflow: bool,
    placement: MenuPlacement,
) -> impl IntoElement {
    let aligned_row = placement.selected_row;
    let below_offset = placement.below_offset;
    let choice = placement.choice;
    let disabled = computed(disabled);
    let active = control_active();
    let anchor = element_ref();
    let open = signal(false);
    let activity = use_context::<NativeMenuActivity>();
    let element_owner = Owner::current();
    let focused = signal(false);
    anchor.on_focus_in(move || focused.set(true));
    anchor.on_focus_out(move || focused.set(false));
    let show = std::rc::Rc::new(move || {
        if disabled.get_untracked() {
            return;
        }
        let choice = choice.clone();
        let items = items();
        element_owner.run(|| {
            spawn(async move {
                if let Some(measurement) = anchor.measure().await {
                    let _tracking = NativeMenuTrackingGuard::new(activity, open);
                    if let Some(choice) = choice {
                        let labels = (choice.labels)();
                        let selected = aligned_row.map(|row| row.get_untracked());
                        #[cfg(target_os = "macos")]
                        {
                            let options = labels
                                .into_iter()
                                .enumerate()
                                .map(|(value, title)| super::commands_macos::ChoiceOption {
                                    value,
                                    title,
                                    enabled: true,
                                    separator_before: false,
                                })
                                .collect();
                            match super::commands_macos::choice_popup(
                                options,
                                selected,
                                small,
                                measurement.frame.size.width.0 as f64,
                                (
                                    (measurement.page.x - px(if small { 11. } else { 12.5 })).0
                                        as f64,
                                    measurement.page.y.0 as f64,
                                ),
                            )
                            .await
                            {
                                Ok(Some(index)) => {
                                    if !disabled.get_untracked() {
                                        (choice.change)(index);
                                    }
                                }
                                Ok(None) => {}
                                Err(error) => eprintln!("Cannot open native choices: {error}"),
                            }
                        }
                        #[cfg(not(target_os = "macos"))]
                        {
                            let items = labels
                                .into_iter()
                                .enumerate()
                                .map(|(index, title)| {
                                    let change = choice.change.clone();
                                    nsh_core::element::desktop::MenuItem::new(title)
                                        .checked(selected == Some(index))
                                        .on_select(move || change(index))
                                })
                                .collect();
                            let _ =
                                nsh_core::element::desktop::popup_menu_at(items, measurement.page)
                                    .await;
                        }
                        return;
                    }
                    #[cfg(target_os = "macos")]
                    let _font_scope = if small || aligned_row.is_some() {
                        match super::commands_macos::PopupFontGuard::with_min_width(
                            items.len(),
                            small,
                            if aligned_row.is_some() {
                                measurement.frame.size.width.0 as f64
                            } else {
                                0.
                            },
                        )
                        .await
                        {
                            Ok(guard) => Some(guard),
                            Err(error) => {
                                eprintln!("Cannot set native popup font: {error}");
                                None
                            }
                        }
                    } else {
                        None
                    };
                    #[cfg(not(target_os = "macos"))]
                    let _ = small;
                    let _ = nsh_core::element::desktop::popup_menu_at(
                        items,
                        Point {
                            x: if aligned_row.is_some() {
                                measurement.page.x - px(if small { 11. } else { 12.5 })
                            } else if toolbar_overflow {
                                measurement.page.x + measurement.frame.size.width - px(7.)
                            } else {
                                measurement.page.x
                                    + px(if cfg!(target_os = "macos") && !borderless {
                                        -5.
                                    } else {
                                        0.
                                    })
                            },
                            y: if let Some(row) = aligned_row {
                                measurement.page.y
                                    - px(row.get_untracked() as f32 * if small { 22. } else { 24. })
                            } else if toolbar_overflow {
                                measurement.page.y + px(9.)
                            } else {
                                measurement.page.y
                                    + measurement.frame.size.height
                                    + px(if cfg!(target_os = "macos") {
                                        (if borderless && !small { 9. } else { 8. }) + below_offset
                                    } else {
                                        0.
                                    })
                            },
                        },
                    )
                    .await;
                }
            });
        });
    });
    if aligned_row.is_some() {
        let show = show.clone();
        effect(move || {
            if focused.get() && !open.get() && !disabled.get() {
                for key in ["up", "down"] {
                    let show = show.clone();
                    on_key_with(
                        key,
                        nsh_core::element::KeyOptions {
                            when: nsh_core::keymap::When::NotEditing,
                            repeat: false,
                            ..Default::default()
                        },
                        move |_| show(),
                    );
                }
            }
        });
    }
    trigger
        .element_ref(&anchor)
        .role(if aligned_row.is_some() {
            AccessibilityRole::Combobox
        } else {
            AccessibilityRole::Button
        })
        .disabled(disabled)
        .focusable(move || !disabled.get())
        .accessibility_state(move || nsh_core::AccessibilityState {
            disabled: disabled.get(),
            expanded: Some(open.get()),
            has_popup: true,
            ..Default::default()
        })
        .focus_visible(|style| {
            style.outline(px(2.)).outline_color(move || {
                if active.get() {
                    theme().ring
                } else {
                    theme().ring.opacity(0.)
                }
            })
        })
        .on_press(move |_: &()| show())
}

pub(super) fn caption(title: impl IntoReactive<SharedString>) -> impl IntoElement {
    text(title)
        .text_size(caption_size)
        .text_color(move || theme().muted_foreground)
}
pub(super) fn row() -> View {
    view().flex_row().items_center().gap_2().flex_wrap()
}
pub(super) fn field_label(key: &str) -> &'static str {
    match key {
        "stop" => return "停止位",
        "flow" => return "流控制",
        "bits" => return "数据位",
        _ => {}
    }
    crate::model::FIELDS
        .iter()
        .chain(crate::model::EDITORS)
        .find(|f| f.0 == key)
        .map_or("", |f| f.1)
}
pub(super) fn option_label(value: &str) -> String {
    match value {
        "" => "自动",
        "none" => "无",
        "all" => "全部",
        "swd" => "SWD",
        "jtag" => "JTAG",
        "automatic" => "按需擦除",
        "skip" => "跳过擦除",
        "full" => "整片擦除",
        "system" => "跟随系统",
        "light" => "浅色",
        "dark" => "深色",
        "utf-8" => "UTF-8",
        "hex" => "HEX",
        "ascii" => "ASCII",
        "gbk" => "GBK",
        "latin-1" => "Latin-1",
        "even" => "偶校验",
        "odd" => "奇校验",
        "software" => "XON/XOFF",
        "hardware" => "RTS/CTS",
        "cr" => "CR",
        "lf" => "LF",
        "crlf" => "CRLF",
        "receive" => "RX",
        "send" => "TX",
        _ => value,
    }
    .into()
}

pub(super) fn body_size() -> Points {
    px(theme().number("body-size").unwrap_or(13.))
}
pub(super) fn small_control_size() -> Points {
    px(theme().number("small-control-size").unwrap_or(11.))
}
pub(super) fn caption_size() -> Points {
    px(theme().number("caption-size").unwrap_or(12.))
}
pub(super) fn native_caption_size() -> Points {
    px(theme().number("native-caption-size").unwrap_or(10.))
}
pub(super) fn small_size() -> Points {
    px(theme().number("small-size").unwrap_or(10.))
}
pub(super) fn mono() -> SharedString {
    theme()
        .font_mono
        .clone()
        .unwrap_or_else(|| "monospace".into())
}
pub(super) fn hairline() -> AdaptiveColor {
    theme().foreground.opacity(0.1)
}
pub(super) fn glass() -> View {
    // NSH only draws the native material on Apple 26+; the other desktop backends need a painted capsule.
    let native = cfg!(target_os = "macos") && os_version().0 >= 26;
    view()
        .h(px(36.))
        .flex_row()
        .items_center()
        .rounded_full()
        .when(native, |view| {
            view.glass_effect(nsh_core::GlassEffect::Regular)
        })
        .when(!native, |view| {
            view.bg(move || theme().color("toolbar-glass").unwrap_or(theme().secondary))
                .border(px(0.5))
                .border_color(move || theme().foreground.opacity(0.13))
                .shadow_sm()
        })
}
pub(super) fn icon_button(
    name: &'static str,
    label: &'static str,
    selected: impl Fn() -> bool + 'static,
    disabled: impl Fn() -> bool + 'static,
    press: impl Fn() + 'static,
) -> impl IntoElement {
    let selected = computed(selected);
    button()
        .on_press(press)
        .selected(selected)
        .text_color(move || {
            if selected.get() {
                theme().primary
            } else {
                theme().foreground.opacity(0.75)
            }
        })
        .disabled(disabled)
        .accessibility_label(label)
        .size(px(22.))
        .p_0()
        .items_center()
        .justify_center()
        .rounded(move || theme().radius)
        .hover(|s| s.bg(move || theme().accent))
        .active(|s| s.bg(move || theme().muted))
        .selected_style(|s| s.text_color(move || theme().primary))
        .focus_visible(|s| s.outline(px(2.)).outline_color(move || theme().ring))
        .disabled_style(|s| s.opacity(0.35))
        .transition_colors()
        .duration(INSTANT)
        .child(super::icons::icon(name).size(px(15.)))
}
pub(super) fn icon_toggle(
    name: &'static str,
    label: impl Fn() -> &'static str + 'static,
    checked: impl Fn() -> bool + 'static,
    disabled: impl Fn() -> bool + 'static,
    change: impl Fn() + 'static,
    width: f32,
) -> impl IntoElement {
    let checked = computed(checked);
    checkbox()
        .checked(checked)
        .disabled(disabled)
        .on_change(move |_: &bool| change())
        .accessibility_label(move || Some(SharedString::from(label())))
        .indicator_style(|style| style.hidden())
        .w(px(width))
        .h(px(20.))
        .hit_slop(nsh_core::geometry::Edges::all(px(5.)))
        .p_0()
        .items_center()
        .justify_center()
        .text_color(move || {
            if checked.get() {
                theme().primary
            } else {
                theme()
                    .color("terminal-glyph")
                    .unwrap_or(theme().muted_foreground)
            }
        })
        .rounded(move || theme().radius)
        .hover(|style| style.bg(move || theme().accent))
        .active(|style| style.bg(move || theme().muted))
        .checked_style(|style| style.text_color(move || theme().primary))
        .focus_visible(|style| style.outline(px(2.)).outline_color(move || theme().ring))
        .disabled_style(|style| style.opacity(0.35))
        .child(
            super::icons::icon(name).size(px(if name == "terminal-composer" {
                15.
            } else {
                14.
            })),
        )
}

pub(super) fn labeled_button(
    name: &'static str,
    label: impl IntoReactive<SharedString>,
    primary: bool,
    disabled: impl Fn() -> bool + 'static,
    press: impl Fn() + 'static,
) -> impl IntoElement {
    let disabled = computed(disabled);
    let active = control_active();
    button()
        .disabled(disabled)
        .on_press(press)
        .h(px(if primary { 28. } else { 24. }))
        .px(px(if primary { 14. } else { 12. }))
        .flex_row()
        .items_center()
        .justify_center()
        .gap(px(5.))
        .rounded(move || if primary { px(14.) } else { px(7.) })
        .border(px(0.))
        .bg(move || emphasized_fill(primary, disabled.get(), active.get(), false))
        .text_color(move || emphasized_ink(primary, disabled.get(), active.get(), false))
        .hover(move |s| {
            s.bg(move || {
                if primary {
                    theme().primary.opacity(0.9)
                } else {
                    theme().accent
                }
            })
        })
        .active(move |s| {
            s.bg(move || {
                if primary {
                    theme().primary.opacity(0.8)
                } else {
                    theme().muted
                }
            })
        })
        .focus_visible(|s| s.outline(px(2.)).outline_color(move || theme().ring))
        .disabled_style(|s| s.opacity(1.))
        .transition_colors()
        .duration(INSTANT)
        .child(super::icons::icon(name))
        .child(text(label).text_size(body_size))
}
pub(super) fn popup_control(
    store: Store,
    key: &'static str,
    values: &'static [&'static str],
    small: bool,
) -> impl IntoElement {
    let h = if small { 20. } else { 24. };
    let locked = move || {
        store.locked(key)
            || (key == "ending" && store.field("format") == "hex" && !store.flag("hex_ending"))
            || (["format", "ending", "checksum"].contains(&key)
                && store.read(|s| {
                    s.active_session()
                        .and_then(|s| s.status.as_ref())
                        .is_some_and(|s| s.job_kind.as_deref() == Some("repeat"))
                }))
    };
    if cfg!(target_os = "macos") {
        let selected_row = computed(move || {
            values
                .iter()
                .position(|value| *value == store.field(key))
                .unwrap_or(0)
        });
        return native_choice_menu(
            view()
                .opacity(move || if locked() { 0.45_f32 } else { 1_f32 })
                .h(px(h))
                .px(px(if small { 8. } else { 10. }))
                .flex_row()
                .items_center()
                .gap(px(6.))
                .rounded(px(if small { 5. } else { 7. }))
                .label(field_label(key))
                .accessibility_value(move || {
                    Some(nsh_core::AccessibilityValue {
                        text: Some(value_label(key, &store.field(key)).into()),
                        ..Default::default()
                    })
                })
                .bg(move || {
                    theme()
                        .color(if key == "length" {
                            "memory-length-control"
                        } else if small {
                            "serial-control"
                        } else {
                            "control"
                        })
                        .unwrap_or(theme().secondary)
                })
                .child(
                    text(move || value_label(key, &store.field(key)))
                        .flex_1()
                        .text_size(move || {
                            if small && !["bits", "stop", "parity", "flow"].contains(&key) {
                                small_control_size()
                            } else {
                                body_size()
                            }
                        }),
                )
                .child(super::icons::icon("select").size(px(12.))),
            locked,
            move || selected_row.get(),
            small,
            move || values.iter().map(|value| value_label(key, value)).collect(),
            move |index| {
                if let Some(value) = values.get(index)
                    && !locked()
                {
                    store.change(|s| s.set(key, (*value).to_owned()));
                }
            },
        )
        .into_element();
    }
    select()
        .items(
            values
                .iter()
                .map(|v| SelectItem::new(*v, value_label(key, v)))
                .collect::<Vec<_>>(),
        )
        .value(move || Some(SharedString::from(store.field(key))))
        .on_change(move |v: &Option<SharedString>| {
            if let Some(v) = v {
                store.change(|s| s.set(key, v.to_string()))
            }
        })
        .disabled(locked)
        .accessibility_label(field_label(key))
        .popup_style(move |style| {
            let style = popup(style);
            if key=="length" { style.w(px(96.)).p(px(5.)).translate_x(px(-12.5)).translate_y(px(-5.)) } else { style }
        })
        .item_class(if key == "length" { "focus:bg-menu-highlight focus:text-primary-foreground selected:bg-menu-highlight selected:text-primary-foreground disabled:opacity-50" } else { "focus:bg-primary focus:text-primary-foreground selected:bg-primary selected:text-primary-foreground disabled:opacity-50" })
        .item_style(move |style| style.rounded(px(if key=="length" {7.} else {5.})))
        .render_item(move |item: &SelectEntry| {
            let value = item.value.clone();
            view().h(px(if key=="length" {24.} else {22.})).px(px(4.)).flex_row().items_center().gap(px(4.))
                .child(super::icons::icon("check").size(px(10.)).opacity(move || if store.field(key)==value.as_str() { 1_f32 } else { 0_f32 }))
                .child(text(item.title.clone()).text_size(body_size))
        })
        .trigger(
            view()
                .opacity(move || if locked() { 0.45_f32 } else { 1_f32 })
                .h(px(h))
                .px(px(if small { 8. } else { 10. }))
                .flex_row()
                .items_center()
                .gap(px(6.))
                .rounded(move || px(if small { 5. } else { 7. }))
                .bg(move || theme().color(if key=="length" { "memory-length-control" } else if small { "serial-control" } else { "control" }).unwrap_or(theme().secondary))
                .child(
                    text(move || value_label(key, &store.field(key)))
                        .flex_1()
                        .text_size(move || {
                            if small && !["bits", "stop", "parity", "flow"].contains(&key) {
                                small_control_size()
                            } else {
                                body_size()
                            }
                        }),
                )
                .child(super::icons::icon("select").size(px(12.))),
        )
        .into_element()
}
pub(super) fn compact_input(
    store: Store,
    key: &'static str,
    label: &'static str,
    width: f32,
) -> impl IntoElement {
    let active = control_active();
    input()
        .value(move || store.field(key))
        .on_change(move |v: &str| store.change(|s| s.set(key, v.into())))
        .label(label)
        .readonly(move || {
            store.locked(key)
                || (key == "interval"
                    && store.read(|s| {
                        s.active_session()
                            .and_then(|s| s.status.as_ref())
                            .is_some_and(|s| s.job_kind.as_deref() == Some("repeat"))
                    }))
        })
        .w(px(width))
        .h(px(20.))
        .px(px(5.))
        .text_size(small_control_size)
        .text_right()
        .bg(move || theme().color("input-well").unwrap_or(theme().background))
        .rounded(move || theme().radius)
        .border_1()
        .border_color(move || theme().color("input-outline").unwrap_or(theme().input))
        .focus(|s| {
            s.outline(px(3.)).outline_color(move || {
                if active.get() {
                    theme().primary.opacity(0.5)
                } else {
                    theme().primary.opacity(0.)
                }
            })
        })
}
pub(super) fn menu_item(
    label: impl IntoReactive<SharedString>,
    press: impl Fn() + 'static,
) -> impl IntoElement {
    nsh_base::menu_item()
        .on_select(press)
        .px_3()
        .py_1()
        .rounded(move || theme().radius)
        .hover(|s| s.bg(move || theme().accent))
        .active(|s| s.bg(move || theme().muted))
        .child(text(label).text_size(body_size))
}
pub(super) fn form_group() -> View {
    view()
        .px(px(10.))
        .bg(move || theme().color("group").unwrap_or(theme().card))
        .rounded(px(10.))
}
pub(super) fn form_row(label: &'static str) -> View {
    view()
        .min_h(px(40.))
        .flex_row()
        .items_center()
        .gap_3()
        .border_b_1()
        .border_color(hairline)
        .child(text(label).text_size(body_size))
}
pub(super) fn toolbar_icon(
    name: &'static str,
    label: &'static str,
    selected: impl Fn() -> bool + 'static,
    disabled: impl Fn() -> bool + 'static,
    press: impl Fn() + 'static,
) -> impl IntoElement {
    button()
        .on_press(press)
        .selected(selected)
        .disabled(disabled)
        .accessibility_label(label)
        .w(px(38.))
        .h(px(34.))
        .p_0()
        .items_center()
        .justify_center()
        .rounded_full()
        .hover(|s| s.bg(move || theme().accent))
        .active(|s| s.bg(move || theme().muted))
        .selected_style(|s| {
            s.bg(move || theme().primary.opacity(0.16))
                .text_color(move || theme().primary)
        })
        .focus_visible(|s| s.outline(px(2.)).outline_color(move || theme().ring))
        .disabled_style(|s| s.opacity(0.35))
        .transition_colors()
        .duration(INSTANT)
        .child({
            let glyph = super::icons::icon(name);
            if name == "memory" {
                glyph
                    .size(px(20.))
                    .scale_x(35. / 34.)
                    .scale_y(0.875)
                    .translate_x(px(0.25))
                    .translate_y(px(0.5))
            } else {
                glyph.size(px(18.))
            }
        })
}

pub(super) fn value_label(key: &str, value: &str) -> String {
    if key == "length" {
        return value.parse::<u64>().map_or_else(
            |_| format!("{value} B"),
            |number| {
                let digits = number.to_string();
                let mut grouped = String::new();
                for (index, digit) in digits.chars().enumerate() {
                    if index > 0 && (digits.len() - index).is_multiple_of(3) {
                        grouped.push(',');
                    }
                    grouped.push(digit);
                }
                format!("{grouped} B")
            },
        );
    }
    if key == "ending" && value == "none" {
        return "无行尾".into();
    }
    if key == "checksum" {
        return match value {
            "none" => "无校验",
            "sum8" => "SUM8",
            "xor8" => "XOR8",
            "oddParity" => "奇校验",
            "evenParity" => "偶校验",
            "crc16Modbus" => "CRC16/MODBUS",
            "crc16Ccitt" => "CRC16/CCITT",
            "crc32" => "CRC32",
            "lrc" => "LRC",
            _ => value,
        }
        .into();
    }
    option_label(value)
}

pub(super) fn draft_input(
    label: &'static str,
    read: impl Fn() -> String + 'static,
    write: impl Fn(&str) + 'static,
) -> Input {
    let active = control_active();
    input()
        .label(label)
        .value(read)
        .on_change(write)
        .h(px(24.))
        .px(px(5.))
        .text_size(body_size)
        .rounded(move || theme().radius)
        .border_1()
        .border_color(move || theme().color("input-outline").unwrap_or(theme().input))
        .bg(move || theme().color("input-well").unwrap_or(theme().background))
        .text_color(move || theme().foreground)
        .caret_color(|| color(theme().primary))
        .placeholder_color(|| color(theme().muted_foreground))
        .focus(|s| {
            s.outline(px(3.)).outline_color(move || {
                if active.get() {
                    theme().color("input-focus-ring").unwrap_or(theme().ring)
                } else {
                    theme().ring.opacity(0.)
                }
            })
        })
}
pub(super) fn draft_toggle(
    label: &'static str,
    checked: impl Fn() -> bool + 'static,
    change: impl Fn(bool) + 'static,
) -> impl IntoElement {
    let active = control_active();
    let checked = computed(checked);
    checkbox()
        .checked(checked)
        .on_change(move |v: &bool| change(*v))
        .accessibility_label(label)
        .flex_row()
        .items_center()
        .gap(px(6.))
        .indicator_style(move |s| {
            s.size(px(16.))
                .rounded(px(4.))
                .items_center()
                .justify_center()
                .bg(move || theme().color("checkbox").unwrap_or(theme().secondary))
                .checked(|s| {
                    s.bg(move || {
                        if active.get() {
                            theme().primary
                        } else {
                            theme()
                                .color("selection-inactive")
                                .unwrap_or(theme().secondary)
                        }
                    })
                })
        })
        .indicator(
            super::icons::icon("check")
                .size(px(11.))
                .text_color(move || theme().primary_foreground)
                .opacity(move || if checked.get() { 1_f32 } else { 0_f32 }),
        )
        .focus_visible(|s| s.outline(px(2.)).outline_color(move || theme().ring))
        .child(text(label).text_size(body_size))
}

pub(super) fn prominent_button(
    name: &'static str,
    label: impl IntoReactive<SharedString>,
    disabled: impl Fn() -> bool + 'static,
    press: impl Fn() + 'static,
) -> impl IntoElement {
    styled_commit_button(name, label, disabled, press, true)
}

fn styled_commit_button(
    name: &'static str,
    label: impl IntoReactive<SharedString>,
    disabled: impl Fn() -> bool + 'static,
    press: impl Fn() + 'static,
    prominent: bool,
) -> impl IntoElement {
    let disabled = computed(disabled);
    let active = control_active();
    let variant = if prominent {
        Variant::Primary
    } else {
        Variant::DefaultAction
    };
    button()
        .disabled(disabled)
        .on_press(press)
        .h(px(24.))
        .min_w(px(88.))
        .px_3()
        .flex_row()
        .items_center()
        .justify_center()
        .gap(px(5.))
        .rounded(px(7.))
        .bg(move || {
            emphasized_fill(
                true,
                disabled.get(),
                active.get(),
                matches!(variant, Variant::DefaultAction),
            )
        })
        .text_color(move || {
            emphasized_ink(
                true,
                disabled.get(),
                active.get(),
                matches!(variant, Variant::DefaultAction),
            )
        })
        .hover(|s| s.bg(move || theme().primary.opacity(0.9)))
        .active(|s| s.bg(move || theme().primary.opacity(0.8)))
        .focus_visible(|s| s.outline(px(2.)).outline_color(move || theme().ring))
        .disabled_style(|s| s.opacity(1.))
        .child(super::icons::icon(name).size(px(15.)))
        .child(text(label).text_size(body_size))
}

pub(super) fn search_field(
    prompt: &'static str,
    read: impl Fn() -> String + 'static,
    write: impl Fn(&str) + 'static,
    reference: ElementRef,
) -> View {
    let active = control_active();
    let read = stored(read);
    let write = stored(write);
    let has_text = move || read.with(|read| !read().is_empty());
    view()
        .h(px(36.))
        .pl(px(12.))
        .flex_row()
        .items_center()
        .gap(px(4.))
        .rounded_full()
        .focus_within(|view| {
            view.outline(px(3.5)).outline_color(move || {
                if active.get() {
                    theme().primary.opacity(0.5)
                } else {
                    theme().primary.opacity(0.)
                }
            })
        })
        .child(
            super::icons::icon("search")
                .size(px(16.))
                .text_color(move || theme().foreground.opacity(0.7)),
        )
        .child(
            input()
                .element_ref(&reference)
                .value(move || read.with(|read| read()))
                .on_change(move |value: &str| write.with(|write| write(value)))
                .label(prompt)
                .placeholder(prompt)
                .submit_behavior(SubmitBehavior::Submit)
                .flex_1()
                .min_w_0()
                .text_size(body_size)
                .placeholder_color(|| color(theme().muted_foreground))
                .caret_color(|| color(theme().primary)),
        )
        .child(when(has_text, move || {
            button()
                .on_press(move || {
                    write.with(|write| write(""));
                    reference.focus();
                })
                .accessibility_label("清除搜索内容")
                .size(px(36.))
                .p_0()
                .items_center()
                .justify_center()
                .rounded_full()
                .hover(|view| view.opacity(0.8))
                .active(|view| view.opacity(0.65))
                .focus_visible(|view| view.outline(px(2.)).outline_color(move || theme().ring))
                .child(
                    view()
                        .size(px(13.))
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .bg(move || theme().muted_foreground)
                        .text_color(move || theme().background)
                        .child(super::icons::icon("close").size(px(9.))),
                )
        }))
}
