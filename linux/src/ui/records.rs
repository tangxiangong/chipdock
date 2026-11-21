//! The serial record-analysis sheet. Selection stays attached to a frame's sequence.
use super::*;
use chipdock_core::serial::{Direction, SerialFrameEvent};
use nsh_base::{
    DataTableClasses, DataTableColumn, DataTableContextMenu, DataTableHeaderCell, DataTablePress,
    DataTableRows, DataTableSelection, DataTableSort, DataTableSortDirection, data_table,
};
use nsh_core::element::desktop::{MenuItem, popup_menu};
use std::cmp::Ordering;

#[derive(Clone, PartialEq)]
struct Record {
    sequence: u64,
    timestamp: String,
    time: String,
    receive: bool,
    source: String,
    length: usize,
    hex: String,
    text: String,
    checksum: &'static str,
    detail: String,
}
impl Record {
    fn from_frame(frame: &SerialFrameEvent) -> Self {
        let checks = frame
            .parsed
            .iter()
            .filter_map(|record| record.checksum_valid)
            .collect::<Vec<_>>();
        Self {
            sequence: frame.sequence,
            timestamp: frame.timestamp.clone(),
            time: time_label(&frame.timestamp),
            receive: matches!(frame.direction, Direction::Receive),
            source: frame.source.clone(),
            length: frame.bytes.len(),
            hex: frame.hex.clone(),
            text: frame.text.utf8.clone(),
            checksum: if checks.is_empty() {
                "—"
            } else if checks.iter().any(|valid| !valid) {
                "失败"
            } else {
                "通过"
            },
            detail: serde_json::to_string_pretty(frame).unwrap_or_default(),
        }
    }
    fn direction(&self) -> &'static str {
        if self.receive { "RX" } else { "TX" }
    }
    fn copy_text(&self) -> String {
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            self.sequence,
            self.timestamp,
            if self.receive { "receive" } else { "send" },
            self.source,
            self.hex,
            self.text
        )
    }
}

#[derive(Clone, Copy)]
struct RecordSheet {
    store: Store,
    session: Option<u64>,
    rows: Memo<Vec<Record>>,
    selection: Signal<Option<u64>>,
    sort: Signal<Option<DataTableSort>>,
}

impl RecordSheet {
    fn selected(&self) -> Option<Record> {
        self.selection.get().and_then(|id| {
            self.rows
                .with(|rows| rows.iter().find(|row| row.sequence == id).cloned())
        })
    }
}

/// Traffic included by the record filter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PropEnum)]
pub enum RecordDirection {
    #[default]
    All,
    Receive,
    Send,
}

#[derive(Debug, Clone, PartialEq, Props)]
pub struct RecordSheetProps {
    /// Case-insensitive text, byte or source filter.
    pub query: Prop<String>,
    /// Selected traffic direction.
    pub direction: Prop<RecordDirection>,
    /// Record sheet controls and native table.
    pub children: Prop<Children>,
}

#[derive(Debug, Clone, PartialEq, Event)]
pub enum RecordSheetEvent {
    /// Number of records matching the current filter.
    #[args(count)]
    CountChange(u32),
}

/// Filters and sorts the captured serial session in Rust, keeping frame identities stable.
#[component]
pub fn SerialRecordSheet(props: RecordSheetProps, cx: Scope<RecordSheetEvent>) -> impl IntoElement {
    let (store, _) = workbench_context();
    provide_context(components::ModalControls);
    let session = store
        .tool_session
        .get_untracked()
        .or(store.read(|s| s.active));
    let selection = signal(None::<u64>);
    let mut first_sort = DataTableSort::default();
    first_sort.key = "sequence".into();
    let sort = signal(Some(first_sort));
    let rows = computed(move || {
        let query = props.query.get().to_lowercase();
        let direction = props.direction.get();
        let mut rows = store.read(|s| {
            session
                .and_then(|id| s.sessions.get(&id))
                .map(|session| {
                    session
                        .frames
                        .iter()
                        .filter(|frame| frame.sequence > session.cleared)
                        .map(Record::from_frame)
                        .filter(|frame| {
                            (direction == RecordDirection::All
                                || frame.receive == (direction == RecordDirection::Receive))
                                && (query.is_empty()
                                    || format!("{} {} {}", frame.hex, frame.source, frame.text)
                                        .to_lowercase()
                                        .contains(&query))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        });
        if let Some(sort) = sort.get() {
            rows.sort_by(|a, b| compare(a, b, &sort));
        }
        rows
    });
    let count = computed(move || rows.with(Vec::len) as u32);
    effect(move || cx.emit(RecordSheetEvent::CountChange(count.get())));
    provide_context(RecordSheet {
        store,
        session,
        rows,
        selection,
        sort,
    });
    view().child(props.children)
}

/// Virtualized table with native selection, sorting, header controls and context menu.
#[component]
pub fn SerialRecordTable() -> impl IntoElement {
    let sheet = use_context::<RecordSheet>().expect("SerialRecordTable requires SerialRecordSheet");
    let RecordSheet {
        store,
        session,
        rows,
        selection,
        sort,
    } = sheet;
    let selected = move || sheet.selected();
    let data = computed(move || {
        rows.with(|rows| {
            DataTableRows::new(rows.len())
                .texts(
                    "sequence",
                    rows.iter().map(|row| number_label(row.sequence)),
                )
                .texts("time", rows.iter().map(|row| &row.time))
                .texts("direction", rows.iter().map(Record::direction))
                .texts("source", rows.iter().map(|row| &row.source))
                .texts(
                    "length",
                    rows.iter().map(|row| number_label(row.length as u64)),
                )
                .texts("hex", rows.iter().map(|row| &row.hex))
                .texts("text", rows.iter().map(|row| &row.text))
                .texts("checksum", rows.iter().map(|row| row.checksum))
        })
    });
    view()
        .relative()
        .flex_1()
        .min_h_0()
        .overflow_hidden()
        .child(
            data_table()
                .columns(columns())
                .data(data)
                .sort(sort)
                .on_sort_change(move |value: &Option<DataTableSort>| sort.set(value.clone()))
                .render_header_cell(move |header: &DataTableHeaderCell| {
                    header_cell(header.clone(), sort)
                })
                .font_size(body_size)
                .row_height(px(24.))
                .header_height(px(28.))
                .column_selectable(false)
                .resizable_columns(true)
                .movable_columns(false)
                .stripe(true)
                .accessibility_label("串口记录表")
                .classes(table_classes())
                .selection(move || {
                    selection.get().and_then(|id| {
                        rows.with(|rows| rows.iter().position(|row| row.sequence == id))
                            .map(|index| {
                                let mut selection = DataTableSelection::default();
                                selection.row = Some(index as u32);
                                selection
                            })
                    })
                })
                .on_selection_change(move |next: &Option<DataTableSelection>| {
                    selection.set(next.as_ref().and_then(|next| {
                        next.row.and_then(|row| {
                            rows.with_untracked(|rows| rows.get(row as usize).map(|r| r.sequence))
                        })
                    }));
                })
                .on_row_press(move |press: &DataTablePress| {
                    selection.set(rows.with_untracked(|rows| {
                        rows.get(press.row as usize).map(|row| row.sequence)
                    }));
                })
                .on_context_menu(move |press: &DataTableContextMenu| {
                    if let Some(row) = press.row {
                        selection.set(
                            rows.with_untracked(|rows| {
                                rows.get(row as usize).map(|row| row.sequence)
                            }),
                        );
                    }
                    let has_selection = selected().is_some();
                    let mut items = vec![
                        MenuItem::new("复制")
                            .disabled(!has_selection)
                            .on_select(move || {
                                if let Some(frame) = selected() {
                                    set_clipboard(frame.copy_text());
                                }
                            }),
                        MenuItem::new("复制记录详情")
                            .disabled(!has_selection)
                            .on_select(move || {
                                if let Some(frame) = selected() {
                                    set_clipboard(frame.detail);
                                }
                            }),
                        MenuItem::separator(),
                    ];
                    for format in ["json", "txt", "csv", "bin"] {
                        items.push(
                            MenuItem::new(format!("导出 {}…", format.to_uppercase())).on_select(
                                move || export(store, session, &rows.get_untracked(), format),
                            ),
                        );
                    }
                    drop(popup_menu(items));
                })
                .flex_1()
                .min_h_0()
                .ml(px(10.)),
        )
        .child(when(move || rows.with(Vec::is_empty), empty_stripes))
        .child(
            view()
                .absolute()
                .top(px(27.))
                .left_0()
                .right_0()
                .h(px(1.))
                .bg(hairline)
                .pointer_events(nsh_core::PointerEvents::None)
                .accessibility_elements_hidden(true),
        )
}

/// Command offered for the record sheet's captured session.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PropEnum)]
pub enum RecordCommand {
    #[default]
    Copy,
    Locate,
    Close,
}

#[derive(Debug, Clone, PartialEq, Props)]
pub struct RecordActionProps {
    /// Command to run for the sheet's captured serial session.
    pub command: Prop<RecordCommand>,
}

/// ChipDock's standard action button bound to the native record selection.
#[component]
pub fn SerialRecordAction(props: RecordActionProps) -> impl IntoElement {
    let sheet =
        use_context::<RecordSheet>().expect("SerialRecordAction requires SerialRecordSheet");
    let RecordSheet { store, session, .. } = sheet;
    let selected = move || sheet.selected();
    match props.command.get() {
        RecordCommand::Copy => action(
            "复制",
            Variant::Default,
            move || selected().is_none(),
            move || {
                if let Some(frame) = selected() {
                    set_clipboard(frame.copy_text());
                }
            },
        )
        .into_element(),
        RecordCommand::Locate => action(
            "在终端定位",
            Variant::Default,
            move || selected().is_none(),
            move || {
                if let (Some(id), Some(frame)) = (session, selected()) {
                    store.change(|s| {
                        s.switch_session(id);
                        s.set("filter", String::new());
                        s.set("direction", "all".into());
                        s.flag("auto_scroll", false);
                    });
                    store.jump.set(Some(frame.sequence));
                    tools::close(store);
                }
            },
        )
        .into_element(),
        RecordCommand::Close => action(
            "完成",
            Variant::Default,
            || false,
            move || tools::close(store),
        )
        .into_element(),
    }
}

pub(super) fn register(registry: &mut Registry) {
    registry
        .register::<SerialRecordSheet>()
        .register::<SerialRecordTable>()
        .register::<SerialRecordAction>();
}

fn compare(a: &Record, b: &Record, sort: &DataTableSort) -> Ordering {
    let order = match sort.key.as_ref() {
        "sequence" => a.sequence.cmp(&b.sequence),
        "time" => a.timestamp.cmp(&b.timestamp),
        "direction" => b.receive.cmp(&a.receive),
        "source" => a.source.cmp(&b.source),
        "length" => a.length.cmp(&b.length),
        "hex" => a.hex.cmp(&b.hex),
        "text" => a.text.cmp(&b.text),
        "checksum" => a.checksum.cmp(b.checksum),
        _ => a.sequence.cmp(&b.sequence),
    }
    .then_with(|| a.sequence.cmp(&b.sequence));
    if sort.direction == DataTableSortDirection::Descending {
        order.reverse()
    } else {
        order
    }
}
fn number_label(value: u64) -> String {
    let digits = value.to_string();
    let mut label = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            label.push(',');
        }
        label.push(digit);
    }
    label
}
fn time_label(timestamp: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|time| {
            time.with_timezone(&chrono::Local)
                .format("%H:%M:%S%.3f")
                .to_string()
        })
        .unwrap_or_else(|_| timestamp.into())
}
fn columns() -> Vec<DataTableColumn> {
    [
        ("sequence", "序号", 115.),
        ("time", "时间", 117.),
        ("direction", "方向", 117.),
        ("source", "来源", 117.),
        ("length", "长度", 117.),
        ("hex", "HEX", 117.),
        ("text", "文本", 117.),
        ("checksum", "校验", 114.),
    ]
    .into_iter()
    .map(|(key, title, width)| {
        let mut column = DataTableColumn::default();
        column.key = key.into();
        column.title = Some(title.into());
        column.width = Some(px(width));
        column.min_width = Some(px(60.));
        // Sorting is a native-style full header action, rather than NSH's default paired glyph button.
        column.sortable = false;
        column
    })
    .collect()
}
fn table_classes() -> DataTableClasses {
    let mut classes = DataTableClasses::default();
    classes.header = Some("bg-background".into());
    classes.header_cell = Some("p-0 text-foreground".into());
    classes.cell = Some("px-2".into());
    classes.row_odd = Some("bg-foreground/4".into());
    classes.row_selected = Some("bg-primary/15".into());
    classes
}
fn header_cell(
    header: DataTableHeaderCell,
    sort: Signal<Option<DataTableSort>>,
) -> impl IntoElement {
    let column = header.column;
    let key = header.key;
    let title = header.title;
    let direction = header.sort;
    let label = format!(
        "{}{}",
        title,
        match direction {
            Some(DataTableSortDirection::Ascending) => "，升序",
            Some(DataTableSortDirection::Descending) => "，降序",
            None => "",
        }
    );
    nsh_base::button()
        .accessibility_label(label)
        .on_press(move || {
            sort.update(|current| {
                let descending = current.as_ref().is_some_and(|s| {
                    s.key == key && s.direction == DataTableSortDirection::Ascending
                });
                let mut next = DataTableSort::default();
                next.key = key.clone();
                next.direction = if descending {
                    DataTableSortDirection::Descending
                } else {
                    DataTableSortDirection::Ascending
                };
                *current = Some(next);
            })
        })
        .relative()
        .h(px(28.))
        .w_full()
        .p_0()
        .pl(px(if column == 0 { 8. } else { 10. }))
        .pr(px(5.))
        .flex_row()
        .items_center()
        .gap_2()
        .focus_visible(|s| s.outline(px(1.)).outline_color(move || theme().ring))
        .child(
            view()
                .absolute()
                .left_0()
                .top(px(6.))
                .bottom(px(6.))
                .w(px(1.))
                .bg(move || theme().foreground.opacity(0.18)),
        )
        .child(
            text(title)
                .text_size(small_control_size)
                .font_weight(if direction.is_some() {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::NORMAL
                })
                .text_color(move || {
                    theme()
                        .color("record-header-foreground")
                        .unwrap_or(theme().foreground)
                }),
        )
        .child(view().flex_1())
        .child(direction.map(|direction| {
            icons::icon(if direction == DataTableSortDirection::Ascending {
                "up"
            } else {
                "down"
            })
            .size(px(13.))
            .translate_y(px(0.5))
            .text_color(move || theme().muted_foreground)
        }))
}
fn empty_stripes() -> View {
    // NSTableView paints the unused body with the same 24 pt alternating rows, starting 5 pt under its header.
    view()
        .absolute()
        .inset_0()
        .pointer_events(nsh_core::PointerEvents::None)
        .accessibility_elements_hidden(true)
        .children((0..7).map(|index| {
            view()
                .absolute()
                .left_0()
                .right_0()
                .top(px(33. + index as f32 * 48.))
                .h(px(24.))
                .bg(move || theme().foreground.opacity(0.04))
        }))
}
fn export(store: Store, session: Option<u64>, rows: &[Record], format: &'static str) {
    let bytes = store.read(|s| {
        let frames = rows
            .iter()
            .filter_map(|row| {
                session
                    .and_then(|id| s.sessions.get(&id))
                    .and_then(|session| {
                        session.frames.iter().find(|frame| {
                            frame.sequence == row.sequence && frame.sequence > session.cleared
                        })
                    })
            })
            .collect::<Vec<_>>();
        crate::presentation::records(&frames, format, "utf-8")
    });
    match bytes {
        Ok(bytes) => store.export(&format!("ChipDock-records.{format}"), bytes),
        Err(error) => store.report(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn record_lengths_sort_numerically_and_sequences_break_ties() {
        let frame = |sequence, length| Record {
            sequence,
            timestamp: String::new(),
            time: String::new(),
            receive: true,
            source: String::new(),
            length,
            hex: String::new(),
            text: String::new(),
            checksum: "—",
            detail: String::new(),
        };
        let mut sort = DataTableSort::default();
        sort.key = "length".into();
        assert_eq!(compare(&frame(1, 2), &frame(2, 10), &sort), Ordering::Less);
        assert_eq!(compare(&frame(1, 10), &frame(2, 10), &sort), Ordering::Less);
        sort.direction = DataTableSortDirection::Descending;
        assert_eq!(
            compare(&frame(1, 2), &frame(2, 10), &sort),
            Ordering::Greater
        );
    }
}
