//! The native recent-firmware list: selection is separate from opening a document.
use super::*;
use nsh_base::{
    DataTable, DataTableApi, DataTableCell, DataTableClasses, DataTableColumn,
    DataTableContextMenu, DataTablePress, DataTableRows, DataTableSelection, data_table,
};
use nsh_core::element::desktop::{MenuItem, popup_menu_at};
use std::{
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, PartialEq)]
struct RecentFile {
    path: String,
    name: String,
    parent: String,
    exists: bool,
    used: Option<u64>,
}
impl RecentFile {
    fn new(path: String, exists: bool, used: Option<u64>) -> Self {
        let file = Path::new(&path);
        let name = file
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let directory = file.parent().unwrap_or(file);
        let parent = std::env::var("HOME")
            .ok()
            .and_then(|home| {
                directory.strip_prefix(home).ok().map(|suffix| {
                    if suffix.as_os_str().is_empty() {
                        "~".into()
                    } else {
                        format!("~/{}", suffix.to_string_lossy())
                    }
                })
            })
            .unwrap_or_else(|| directory.to_string_lossy().into_owned());
        Self {
            path,
            name,
            parent,
            exists,
            used,
        }
    }
}

pub(super) fn page(store: Store) -> impl IntoElement {
    let refreshed = signal(0_u64);
    nsh_core::element::desktop::current_window().on_focus_change(move |active| {
        if active {
            refreshed.update(|value| *value += 1)
        }
    });
    let files = computed(move || {
        refreshed.get();
        store.read(|state| {
            state
                .preferences
                .recent
                .iter()
                .map(|path| {
                    RecentFile::new(
                        path.clone(),
                        Path::new(path).is_file(),
                        state.preferences.recent_used.get(path).copied(),
                    )
                })
                .collect::<Vec<_>>()
        })
    });
    let selected = signal(None::<String>);
    let clock = signal(now());
    let focused = signal(false);
    let context_open = signal(None::<String>);
    let table = component_ref::<DataTable>();
    let list_ref = element_ref();
    // NSH 0.5.1 keeps pointer selection logically focused; the list frame provides its platform Tab stop.
    list_ref.on_focus_in(move || {
        focused.set(true);
        if let Some(table) = table.api::<DataTableApi>() {
            table.focus();
        }
    });
    list_ref.on_focus_out(move || {
        focused.set(false);
        if let Some(table) = table.api::<DataTableApi>() {
            table.blur();
        }
    });
    effect(move || {
        if !focused.get() || busy(store) {
            return;
        }
        let keys = KeyOptions {
            when: nsh_core::keymap::When::NotEditing,
            ..Default::default()
        };
        on_key_with("enter", keys, move |_| {
            if let Some(path) = selected.get_untracked() {
                open(store, &path);
            }
        });
        for key in ["backspace", "delete"] {
            on_key_with(key, keys, move |_| {
                if let Some(path) = selected.get_untracked() {
                    remove(store, Removal::One(path), selected);
                }
            });
        }
    });
    let width = signal(1_f32);
    let data = computed(move || {
        files.with(|files| {
            DataTableRows::new(files.len()).texts("path", files.iter().map(|file| &file.path))
        })
    });
    when(
        move || files.with(|files| !files.is_empty()),
        move || {
            set_interval(Duration::from_secs(1), move || clock.set(now()));
            view()
                .gap(px(6.))
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap_3()
                        .child(
                            text("最近使用")
                                .text_size(px(11.))
                                .text_color(move || theme().muted_foreground),
                        )
                        .child(view().flex_1())
                        .child(when(
                            move || files.with(|files| files.iter().any(|file| !file.exists)),
                            move || {
                                borderless(
                                    "清除失效记录",
                                    move || busy(store),
                                    move || remove(store, Removal::Missing, selected),
                                )
                            },
                        ))
                        .child(borderless(
                            "清空列表",
                            move || busy(store),
                            move || remove(store, Removal::All, selected),
                        )),
                )
                .child(
                    view()
                        .h(move || px((files.with(Vec::len).min(5) * 30 + 2).max(32) as f32))
                        .relative()
                        .overflow_hidden()
                        .border_1()
                        .border_color(move || theme().foreground.opacity(0.1))
                        .on_layout(move |bounds: &Bounds<Points>| {
                            width.set((bounds.size.width.0 - 2.).max(1.))
                        })
                        .element_ref(&list_ref)
                        .focusable(true)
                        // shortcut: NSH reserves at least 1 pt for a header; clip that unused band until it has a headerless mode.
                        .child(
                            data_table()
                                .component_ref(&table)
                                .columns(move || vec![column(width.get())])
                                .data(data)
                                .header_height(px(1.))
                                .row_height(px(30.))
                                .row_header(false)
                                .row_selectable(move || !busy(store))
                                .column_selectable(false)
                                .cell_selectable(false)
                                .loop_selection(false)
                                .resizable_columns(false)
                                .horizontal_scrollbar(false)
                                .movable_columns(false)
                                .stripe(true)
                                .font_size(body_size)
                                .accessibility_label("最近使用的固件")
                                .classes(classes())
                                .selection(move || {
                                    selected_index(&files.get(), selected.get().as_deref()).map(
                                        |row| {
                                            let mut value = DataTableSelection::default();
                                            value.row = Some(row);
                                            value
                                        },
                                    )
                                })
                                .on_selection_change(move |value: &Option<DataTableSelection>| {
                                    if busy(store) {
                                        return;
                                    }
                                    selected.set(
                                        value.as_ref().and_then(|value| value.row).and_then(
                                            |row| {
                                                files.with_untracked(|files| {
                                                    files
                                                        .get(row as usize)
                                                        .map(|file| file.path.clone())
                                                })
                                            },
                                        ),
                                    );
                                })
                                .on_row_press(move |press: &DataTablePress| {
                                    if busy(store) {
                                        return;
                                    }
                                    if let Some(file) = files.with_untracked(|files| {
                                        files.get(press.row as usize).cloned()
                                    }) {
                                        list_ref.focus();
                                        selected.set(Some(file.path.clone()));
                                        if press.clicks == 2 {
                                            open(store, &file.path);
                                        }
                                    }
                                })
                                .on_context_menu(move |press: &DataTableContextMenu| {
                                    if busy(store) {
                                        return;
                                    }
                                    let file = press.row.and_then(|row| {
                                        files.with_untracked(|files| {
                                            files.get(row as usize).cloned()
                                        })
                                    });
                                    if let Some(file) = file {
                                        let open_path = file.path.clone();
                                        let reveal = file.path.clone();
                                        let remove_path = file.path.clone();
                                        let missing = !file.exists;
                                        context_open.set(Some(file.path));
                                        let request = popup_menu_at(
                                            vec![
                                                MenuItem::new("打开")
                                                    .disabled(missing)
                                                    .on_select(move || open(store, &open_path)),
                                                MenuItem::new(if cfg!(target_os = "macos") {
                                                    "在访达中显示"
                                                } else {
                                                    "在文件夹中显示"
                                                })
                                                .disabled(missing)
                                                .on_select(move || {
                                                    if busy(store) {
                                                        return;
                                                    }
                                                    let path = reveal.clone();
                                                    store.spawn(async move {
                                                        if let Err(error) = reveal_path(path).await
                                                        {
                                                            store.report(error)
                                                        }
                                                    });
                                                }),
                                                MenuItem::separator(),
                                                MenuItem::new("从最近使用中移除").on_select(
                                                    move || {
                                                        remove(
                                                            store,
                                                            Removal::One(remove_path.clone()),
                                                            selected,
                                                        )
                                                    },
                                                ),
                                            ],
                                            Point {
                                                x: press.x,
                                                y: press.y,
                                            },
                                        );
                                        store.spawn(async move {
                                            let _ = request.await;
                                            context_open.try_set(None);
                                        });
                                    }
                                })
                                .render_cell(move |cell: &DataTableCell| {
                                    let file = files.with_untracked(|files| {
                                        files.get(cell.row as usize).cloned()
                                    });
                                    view().child(file.map(|file| {
                                        let path = file.path.clone();
                                        let outline_path = path.clone();
                                        let current = computed(move || {
                                            files.with(|files| {
                                                files.iter().find(|file| file.path == path).cloned()
                                            })
                                        });
                                        file_row(
                                            store,
                                            file,
                                            current,
                                            cell.row % 2 == 1,
                                            clock,
                                            selected,
                                            focused,
                                        )
                                        .relative()
                                        .child(when(
                                            move || {
                                                context_open.with(|path| {
                                                    path.as_ref() == Some(&outline_path)
                                                })
                                            },
                                            || {
                                                view()
                                                    .absolute()
                                                    .inset_0()
                                                    .border_2()
                                                    .border_color(move || {
                                                        theme()
                                                            .color("selection-active")
                                                            .unwrap_or(theme().primary)
                                                    })
                                                    .pointer_events(nsh_core::PointerEvents::None)
                                            },
                                        ))
                                    }))
                                })
                                .h(
                                    move || {
                                        px((files.with(Vec::len).min(5) * 30 + 1).max(31) as f32)
                                    },
                                )
                                .mt(px(-1.)),
                        ),
                )
        },
    )
}

fn column(width: f32) -> DataTableColumn {
    let mut column = DataTableColumn::default();
    column.key = "path".into();
    column.title = Some(SharedString::default());
    column.width = Some(px(width));
    column.min_width = Some(px(1.));
    column
}
fn classes() -> DataTableClasses {
    let mut classes = DataTableClasses::default();
    classes.root = Some("border-0 rounded-none".into());
    classes.header = Some("hidden".into());
    classes.cell = Some("p-0".into());
    classes.row_selected = Some("bg-selection-active text-primary-foreground".into());
    classes
}
fn file_row(
    store: Store,
    file: RecentFile,
    current: Memo<Option<RecentFile>>,
    odd: bool,
    clock: Signal<u64>,
    selection: Signal<Option<String>>,
    focused: Signal<bool>,
) -> View {
    let path = file.path.clone();
    let selected = computed(move || selection.with(|selected| selected.as_ref() == Some(&path)));
    let secondary = move || {
        if selected.get() && focused.get() {
            theme().primary_foreground.opacity(0.8)
        } else {
            theme().muted_foreground
        }
    };
    let missing = move || current.with(|file| file.as_ref().is_none_or(|file| !file.exists));
    let tint_path = file.path.clone();
    let remove_path = file.path.clone();
    let label = format!("移除 {} 的最近使用记录", file.name);
    view()
        .h(px(30.))
        .px(px(6.))
        .flex_row()
        .items_center()
        .gap_2()
        .text_size(body_size)
        .bg(move || {
            if selected.get() && focused.get() {
                theme().color("selection-active").unwrap_or(theme().primary)
            } else if selected.get() {
                theme()
                    .color("selection-inactive")
                    .unwrap_or(theme().secondary)
            } else if odd {
                theme().color("table-stripe").unwrap_or(theme().muted)
            } else {
                theme().background
            }
        })
        .child(view().w(px(16.)).items_center().justify_center().child(
            icons::icon("file").size(px(15.)).text_color(move || {
                if selected.get() && focused.get() {
                    theme().primary_foreground
                } else {
                    file_tint(&tint_path)
                }
            }),
        ))
        .child(
            text(file.name)
                .number_of_lines(1)
                .flex_shrink_0()
                .text_color(move || {
                    if selected.get() && focused.get() {
                        theme().primary_foreground
                    } else if missing() {
                        theme().muted_foreground
                    } else {
                        theme().foreground
                    }
                }),
        )
        .child(
            text(file.parent)
                .flex_1()
                .min_w_0()
                .number_of_lines(1)
                .text_overflow(nsh_core::EllipsizeMode::Head)
                .text_color(secondary),
        )
        .child(
            when(
                move || !missing(),
                move || {
                    text(move || {
                        current
                            .with(|file| file.as_ref().and_then(|file| file.used))
                            .map(|used| relative_time(clock.get(), used))
                            .unwrap_or_default()
                    })
                    .text_size(caption_size)
                    .text_color(secondary)
                },
            )
            .otherwise(move || {
                view()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .text_color(move || theme().color("firmware-tint").unwrap_or(theme().primary))
                    .child(icons::icon("warning-fill").size(px(13.)))
                    .child(text("文件不存在").text_size(caption_size))
            }),
        )
        .child(
            nsh_base::button()
                .accessibility_label(label)
                .disabled(move || busy(store))
                .on_press(move || remove(store, Removal::One(remove_path.clone()), selection))
                .size(px(18.))
                .p_0()
                .items_center()
                .justify_center()
                .text_color(secondary)
                .focus_visible(|s| s.outline(px(2.)).outline_color(move || theme().ring))
                .child(icons::icon("close").size(px(15.))),
        )
}
fn file_tint(path: &str) -> AdaptiveColor {
    match Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("elf" | "axf") => theme().color("firmware-elf").unwrap_or(theme().chart_4),
        Some("bin") => theme().color("firmware-bin").unwrap_or(theme().chart_2),
        _ => theme().color("firmware-tint").unwrap_or(theme().primary),
    }
}
fn borderless(
    label: &'static str,
    disabled: impl Fn() -> bool + 'static,
    press: impl Fn() + 'static,
) -> impl IntoElement {
    nsh_base::button()
        .on_press(press)
        .disabled(disabled)
        .h(px(18.))
        .p_0()
        .justify_center()
        .text_color(move || theme().muted_foreground)
        .hover(|s| s.text_color(move || theme().foreground))
        .active(|s| s.opacity(0.7))
        .disabled_style(|s| s.opacity(0.45))
        .focus_visible(|s| s.outline(px(2.)).outline_color(move || theme().ring))
        .child(text(label).text_size(body_size))
}
fn busy(store: Store) -> bool {
    store.modal.get() || store.read(|state| state.lane_busy(Lane::Probe))
}
fn open(store: Store, path: &str) {
    if busy(store)
        || !store.read(|state| state.preferences.recent.iter().any(|entry| entry == path))
        || !Path::new(path).is_file()
    {
        return;
    }
    store.change(|state| state.open_recent(path));
}
enum Removal {
    One(String),
    Missing,
    All,
}
pub(super) fn remove_one(store: Store, path: &str) {
    remove_history(store, Removal::One(path.into()));
}
pub(super) fn remove_missing(store: Store) {
    remove_history(store, Removal::Missing);
}
pub(super) fn remove_all(store: Store) {
    remove_history(store, Removal::All);
}
fn remove(store: Store, removal: Removal, selection: Signal<Option<String>>) {
    if remove_history(store, removal)
        && let Some(path) = selection.get_untracked()
        && !store.read(|state| state.preferences.recent.contains(&path))
    {
        selection.set(None);
    }
}
fn remove_history(store: Store, removal: Removal) -> bool {
    if busy(store) {
        return false;
    }
    let result = store.change(|state| -> anyhow::Result<()> {
        let previous = state.preferences.clone();
        state.preferences.recent.retain(|path| match &removal {
            Removal::One(removed) => path != removed,
            Removal::Missing => Path::new(path).is_file(),
            Removal::All => false,
        });
        let paths = &state.preferences.recent;
        state
            .preferences
            .recent_bases
            .retain(|path, _| paths.contains(path));
        state
            .preferences
            .recent_used
            .retain(|path, _| paths.contains(path));
        if let Err(error) = state.save() {
            state.preferences = previous.clone();
            if let Some(session) = state.active.and_then(|id| state.sessions.get_mut(&id)) {
                session.preferences = previous;
            }
            return Err(error);
        }
        Ok(())
    });
    match result {
        Ok(()) => true,
        Err(error) => {
            store.report(error);
            false
        }
    }
}
fn selected_index(files: &[RecentFile], selected: Option<&str>) -> Option<u32> {
    let selected = selected?;
    files
        .iter()
        .position(|file| file.path == selected)
        .map(|index| index as u32)
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |value| value.as_secs())
}
fn relative_time(now: u64, used: u64) -> String {
    use chrono::TimeZone;
    let dates = i64::try_from(now)
        .ok()
        .zip(i64::try_from(used).ok())
        .and_then(|(now, used)| {
            chrono::Local
                .timestamp_opt(now, 0)
                .single()
                .zip(chrono::Local.timestamp_opt(used, 0).single())
        });
    dates
        .map(|(now, used)| relative_time_at(now.naive_local(), used.naive_local()))
        .unwrap_or_else(|| "现在".into())
}

#[derive(Clone, Copy)]
enum RelativeUnit {
    Year,
    Month,
    Week,
    Day,
    Hour,
    Minute,
    Second,
}

fn relative_component(
    from: chrono::NaiveDateTime,
    to: chrono::NaiveDateTime,
) -> (RelativeUnit, i64) {
    use chrono::{Datelike, Months};
    let direction = if to >= from { 1 } else { -1 };
    let mut months =
        ((to.year() - from.year()) * 12 + to.month() as i32 - from.month() as i32).unsigned_abs();
    let shifted = if direction > 0 {
        from.checked_add_months(Months::new(months))
    } else {
        from.checked_sub_months(Months::new(months))
    };
    if shifted
        .is_some_and(|shifted| (direction > 0 && shifted > to) || (direction < 0 && shifted < to))
    {
        months = months.saturating_sub(1);
    }
    if months >= 12 {
        return (RelativeUnit::Year, i64::from(months / 12) * direction);
    }
    if months > 0 {
        return (RelativeUnit::Month, i64::from(months) * direction);
    }
    let seconds = (to - from).num_seconds().unsigned_abs();
    for (unit, width) in [
        (RelativeUnit::Week, 604800),
        (RelativeUnit::Day, 86400),
        (RelativeUnit::Hour, 3600),
        (RelativeUnit::Minute, 60),
        (RelativeUnit::Second, 1),
    ] {
        if seconds >= width || width == 1 {
            return (unit, (seconds / width) as i64 * direction);
        }
    }
    (RelativeUnit::Second, 0)
}

fn relative_time_at(now: chrono::NaiveDateTime, used: chrono::NaiveDateTime) -> String {
    use chrono::{Datelike, Duration, Months, NaiveDate};
    let (unit, initial) = relative_component(now, used);
    // Match Swift Date.RelativeFormatStyle.named: round clock units and align calendar units.
    let (unit, value) = match unit {
        RelativeUnit::Second | RelativeUnit::Minute | RelativeUnit::Hour => {
            let width = match unit {
                RelativeUnit::Hour => 3600,
                RelativeUnit::Minute => 60,
                _ => 1,
            };
            let seconds = (used - now).num_seconds();
            let rounded = ((seconds.unsigned_abs() + width / 2) / width) as i64 * seconds.signum();
            used.checked_sub_signed(Duration::seconds(rounded * width as i64))
                .map(|from| relative_component(from, used))
                .unwrap_or((unit, initial))
        }
        _ => {
            let date = now.date();
            let start = match unit {
                RelativeUnit::Year => NaiveDate::from_ymd_opt(date.year(), 1, 1),
                RelativeUnit::Month => date.with_day(1),
                RelativeUnit::Week => date.checked_sub_signed(Duration::days(i64::from(
                    date.weekday().num_days_from_monday(),
                ))),
                _ => Some(date),
            };
            let end = start.and_then(|start| match unit {
                RelativeUnit::Year => start.checked_add_months(Months::new(12)),
                RelativeUnit::Month => start.checked_add_months(Months::new(1)),
                RelativeUnit::Week => start.checked_add_signed(Duration::days(7)),
                _ => start.succ_opt(),
            });
            let boundary = if used >= now {
                start.and_then(|date| date.and_hms_opt(0, 0, 0))
            } else {
                end.and_then(|date| date.and_hms_opt(0, 0, 0))
                    .and_then(|date| date.checked_sub_signed(Duration::seconds(1)))
            };
            boundary
                .map(|from| relative_component(from, used))
                .unwrap_or((unit, initial))
        }
    };
    let future = value > 0;
    let amount = value.unsigned_abs();
    match (unit, amount, future) {
        (RelativeUnit::Second, 0, _) => "现在".into(),
        (RelativeUnit::Day, 1, false) => "昨天".into(),
        (RelativeUnit::Day, 2, false) => "前天".into(),
        (RelativeUnit::Day, 1, true) => "明天".into(),
        (RelativeUnit::Day, 2, true) => "后天".into(),
        (RelativeUnit::Week, 1, false) => "上周".into(),
        (RelativeUnit::Week, 1, true) => "下周".into(),
        (RelativeUnit::Month, 1, false) => "上个月".into(),
        (RelativeUnit::Month, 1, true) => "下个月".into(),
        (RelativeUnit::Year, 1, false) => "去年".into(),
        (RelativeUnit::Year, 1, true) => "明年".into(),
        _ => {
            let unit = match unit {
                RelativeUnit::Year => "年",
                RelativeUnit::Month => "个月",
                RelativeUnit::Week => "周",
                RelativeUnit::Day => "天",
                RelativeUnit::Hour => "小时",
                RelativeUnit::Minute => "分钟",
                RelativeUnit::Second => "秒钟",
            };
            format!("{amount}{unit}{}", if future { "后" } else { "前" })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relative_time_matches_current_swift_named_clock_boundaries() {
        let now = chrono::NaiveDate::from_ymd_opt(2026, 11, 15)
            .unwrap()
            .and_hms_opt(1, 26, 15)
            .unwrap();
        for (age, expected) in [
            (0, "现在"),
            (1, "1秒钟前"),
            (59, "59秒钟前"),
            (60, "1分钟前"),
            (89, "1分钟前"),
            (90, "2分钟前"),
            (1231, "21分钟前"),
            (1411, "24分钟前"),
            (3569, "59分钟前"),
            (3570, "1小时前"),
            (5399, "1小时前"),
            (5400, "2小时前"),
            (84600, "昨天"),
            (172800, "前天"),
            (-1, "1秒钟后"),
            (-60, "1分钟后"),
            (-86400, "明天"),
            (-172800, "后天"),
        ] {
            assert_eq!(
                relative_time_at(now, now - chrono::Duration::seconds(age)),
                expected,
                "age {age}"
            );
        }
    }
    #[test]
    fn named_calendar_units_align_with_current_day_week_month_and_year() {
        let now = chrono::NaiveDate::from_ymd_opt(2026, 11, 15)
            .unwrap()
            .and_hms_opt(1, 26, 15)
            .unwrap();
        for (age, expected) in [
            (6 * 86400, "6天前"),
            (6 * 86400 + 8640, "上周"),
            (13 * 86400, "上周"),
            (14 * 86400, "2周前"),
            (30 * 86400, "4周前"),
            (31 * 86400, "上个月"),
            (46 * 86400, "2个月前"),
            (364 * 86400, "去年"),
            (730 * 86400, "2年前"),
            (-7 * 86400, "下周"),
            (-30 * 86400, "下个月"),
            (-365 * 86400, "明年"),
        ] {
            assert_eq!(
                relative_time_at(now, now - chrono::Duration::seconds(age)),
                expected,
                "age {age}"
            );
        }
    }
    #[test]
    fn selection_stays_with_the_path_after_reordering_and_removal() {
        let file = |path: &str| RecentFile::new(path.into(), true, None);
        let first = file("/tmp/first.hex");
        let second = file("/tmp/second.bin");
        assert_eq!(
            selected_index(&[first.clone(), second.clone()], Some(&second.path)),
            Some(1)
        );
        assert_eq!(
            selected_index(&[second.clone(), first], Some(&second.path)),
            Some(0)
        );
        assert_eq!(selected_index(&[], Some(&second.path)), None);
    }
}
