use super::*;
use chipdock_core::models::MemoryRegionKind;

pub(super) fn page(store: Store) -> View {
    crate::ui::commands::watch_window(store, "memory", None);
    let regions = computed(move || {
        store.read(|s| {
            s.layout
                .iter()
                .map(|r| (r.name.clone(), r.kind, r.start, r.end, r.size))
                .collect::<Vec<_>>()
        })
    });
    let bytes = computed(move || {
        store.read(|s| {
            s.memory.as_ref().and_then(|m| {
                crate::model::decode_hex(&m.data_hex)
                    .ok()
                    .map(|bytes| (m.address, bytes))
            })
        })
    });
    let can_read = move || store.read(crate::ui::commands::can_read_memory);
    view()
        .flex_1()
        .min_h_0()
        .child(
            view()
                .px_4()
                .pt(px(14.))
                .pb(px(10.))
                .flex_row()
                .items_center()
                .gap(px(10.))
                .child(text("内存").text_size(px(15.)))
                .child(view().flex_1())
                .child(memory_tool(
                    "export",
                    "导出为二进制",
                    move || store.read(|s| s.memory.is_none() || s.lane_busy(Lane::Probe)),
                    move || export(store),
                ))
                .child(native_borderless_menu_disabled(
                    view()
                        .w(px(21.))
                        .h(px(15.))
                        .items_center()
                        .justify_center()
                        .label("芯片操作")
                        .child(icons::icon("memory-more").size(px(16.))),
                    || false,
                    move || {
                        vec![
                            nsh_core::element::desktop::MenuItem::new("擦除整个芯片…")
                                .disabled(
                                    !crate::ui::commands::probe_idle(store)
                                        || !store.read(Workbench::target_connected),
                                )
                                .on_select(move || {
                                    if crate::ui::commands::probe_idle(store)
                                        && store.read(Workbench::target_connected)
                                    {
                                        erase(store);
                                    }
                                }),
                        ]
                    },
                )),
        )
        .child(
            view()
                .px_4()
                .pb_3()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    draft_input(
                        "起始地址",
                        move || store.field("address"),
                        move |v: &str| store.change(|s| s.set("address", v.into())),
                    )
                    .font_family(mono)
                    .text_size(caption_size)
                    .h(px(23.))
                    .flex_1()
                    .min_w_0()
                    .on_submit(move |_: &str| {
                        if can_read() {
                            store.command(|s| Command::Memory(s.preferences.clone()))
                        }
                    }),
                )
                .child(when(
                    move || crate::model::number(&store.field("address")).is_err(),
                    || {
                        icons::icon("warning")
                            .text_color(move || theme().destructive)
                            .label("地址无效")
                    },
                ))
                .child(view().w(px(95.5)).child(popup_control(
                    store,
                    "length",
                    &["64", "128", "256", "512", "1024", "2048", "4096"],
                    false,
                )))
                .child(view().w(px(73.)).child(labeled_button(
                    "download-circle",
                    "读取",
                    false,
                    move || !can_read(),
                    move || store.command(|s| Command::Memory(s.preferences.clone())),
                ))),
        )
        .child(when(
            move || regions.with(|r| !r.is_empty()),
            move || {
                view()
                    .border_t_1()
                    .border_color(hairline)
                    .h(move || px(regions.with(|r| (r.len() * 24 + 34).min(160) as f32)))
                    .child(
                        text("内存映射")
                            .text_size(caption_size)
                            .text_color(move || theme().muted_foreground)
                            .px_4()
                            .py_2(),
                    )
                    .child(
                        virtual_list(
                            move || regions.with(Vec::len),
                            move |at| {
                                let region =
                                    computed(move || regions.with(|r| r.get(at.index()).cloned()));
                                nsh_base::button()
                                    .on_press(move || {
                                        if let Some(region) = region.get_untracked() {
                                            store.change(|s| {
                                                s.set("address", format!("0x{:08X}", region.2))
                                            })
                                        }
                                    })
                                    .disabled(move || store.read(|s| s.lane_busy(Lane::Probe)))
                                    .h(px(24.))
                                    .px_4()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(6.))
                                    .selected(move || {
                                        region.with(|r| {
                                            r.as_ref().is_some_and(|r| {
                                                crate::model::number(&store.field("address"))
                                                    .is_ok_and(|a| a >= r.2 && a < r.3)
                                            })
                                        })
                                    })
                                    .selected_style(|s| s.bg(move || theme().primary.opacity(0.12)))
                                    .hover(|s| s.bg(move || theme().accent))
                                    .focus_visible(|s| {
                                        s.outline(px(2.)).outline_color(move || theme().ring)
                                    })
                                    .child(view().w(px(3.)).h_3().bg(move || {
                                        region.with(|r| match r.as_ref().map(|r| &r.1) {
                                            Some(MemoryRegionKind::Nvm) => {
                                                theme().color("warning").unwrap_or(theme().primary)
                                            }
                                            Some(MemoryRegionKind::Ram) => theme().chart_2,
                                            _ => theme().muted_foreground,
                                        })
                                    }))
                                    .child(
                                        text(move || {
                                            region.with(|r| {
                                                r.as_ref()
                                                    .map(|r| {
                                                        r.0.clone().unwrap_or_else(|| {
                                                            match r.1 {
                                                                MemoryRegionKind::Nvm => "Flash",
                                                                MemoryRegionKind::Ram => "RAM",
                                                                _ => "存储区域",
                                                            }
                                                            .into()
                                                        })
                                                    })
                                                    .unwrap_or_default()
                                            })
                                        })
                                        .flex_1()
                                        .text_size(body_size),
                                    )
                                    .child(
                                        text(move || {
                                            region.with(|r| {
                                                r.as_ref()
                                                    .map(|r| format!("0x{:08X}", r.2))
                                                    .unwrap_or_default()
                                            })
                                        })
                                        .font_family(mono)
                                        .text_size(caption_size)
                                        .text_color(move || theme().muted_foreground),
                                    )
                                    .child(
                                        text(move || {
                                            region.with(|r| {
                                                r.as_ref()
                                                    .map(|r| byte_label(r.4))
                                                    .unwrap_or_default()
                                            })
                                        })
                                        .w(px(58.))
                                        .text_right()
                                        .text_size(caption_size)
                                        .text_color(move || theme().muted_foreground),
                                    )
                            },
                        )
                        .estimated_item_size(px(24.))
                        .flex_1(),
                    )
            },
        ))
        .child(view().h(px(1.)).bg(hairline))
        .child(
            when(move || bytes.with(Option::is_some), move || hex_dump(bytes)).otherwise(
                move || {
                    super::empty_view(
                        || "memory",
                        "尚未读取",
                        move || {
                            if store.read(Workbench::target_connected) {
                                "输入地址后按回车或点击“读取”"
                            } else {
                                "连接目标后可读取内存"
                            }
                        },
                    )
                },
            ),
        )
}
fn hex_dump(bytes: Memo<Option<(u64, Vec<u8>)>>) -> impl IntoElement {
    let rows = computed(move || {
        bytes.with(|b| {
            b.as_ref()
                .map(|(address, bytes)| {
                    bytes
                        .chunks(8)
                        .enumerate()
                        .map(|(i, b)| (*address + i as u64 * 8, b.to_vec()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
    });
    scroll().flex_1().child(
        view()
            .px_4()
            .pb_2()
            .gap(px(1.))
            .font_family(mono)
            .text_size(px(11.))
            .selectable(true)
            .child(
                view()
                    .flex_row()
                    .gap_3()
                    .py_1()
                    .text_color(move || theme().muted_foreground)
                    .child(text("Offset    ").w(px(66.)))
                    .child(text("00 01 02 03 04 05 06 07").w(px(152.)))
                    .child(text("ASCII")),
            )
            .child(each(
                rows,
                |r: &(u64, Vec<u8>)| r.0,
                move |row, _| {
                    view()
                        .flex_row()
                        .gap_3()
                        .child(
                            text(move || format!("0x{:08X}", row.get().0))
                                .w(px(66.))
                                .text_color(move || theme().muted_foreground),
                        )
                        .child(
                            view()
                                .w(px(152.))
                                .flex_row()
                                .children((0..8).map(move |index| {
                                    text(move || {
                                        row.with(|r| {
                                            r.1.get(index)
                                                .map(|b| format!("{b:02X} "))
                                                .unwrap_or_else(|| "   ".into())
                                        })
                                    })
                                    .text_color(move || {
                                        row.with(|r| {
                                            if r.1.get(index).is_some_and(|b| *b == 0 || *b == 255)
                                            {
                                                theme().muted_foreground.opacity(0.5)
                                            } else {
                                                theme().foreground
                                            }
                                        })
                                    })
                                })),
                        )
                        .child(
                            text(move || {
                                row.with(|r| {
                                    r.1.iter()
                                        .map(|b| {
                                            if (32..127).contains(b) {
                                                *b as char
                                            } else {
                                                '·'
                                            }
                                        })
                                        .collect::<String>()
                                })
                            })
                            .text_color(move || theme().muted_foreground),
                        )
                },
            )),
    )
}
fn export(store: Store) {
    let result = store.read(|s| {
        s.memory.as_ref().map(|m| {
            crate::model::decode_hex(&m.data_hex)
                .map(|bytes| (format!("memory-0x{:08X}.bin", m.address), bytes))
        })
    });
    match result {
        Some(Ok((name, bytes))) => store.export_as(
            &name,
            bytes,
            crate::ui::file_dialogs::ExportKind::DocumentData,
        ),
        Some(Err(e)) => store.report(e),
        None => {}
    }
}
pub(super) fn erase(store: Store) {
    let captured = store.read(|s| s.preferences.clone());
    store.spawn(async move {
        let result = nsh_base::alert_dialog()
            .title(format!("擦除 {} 的全部 Flash？", captured.get("chip")))
            .description("芯片中的固件与数据将被永久删除，此操作无法撤销。")
            .cancel("cancel", "取消")
            .destructive("erase", "擦除全部 Flash")
            .show()
            .await;
        if matches!(result,Ok(answer) if answer.action.as_deref()==Some("erase"))
            && store.read(|s| {
                s.target_connected()
                    && s.preferences.target_key() == captured.target_key()
                    && !s.lane_busy(Lane::Probe)
            })
        {
            store.command(|_| Command::Erase(captured));
        }
    });
}
fn byte_label(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024. * 1024.))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.)
    } else {
        format!("{bytes} B")
    }
}

fn memory_tool(
    name: &'static str,
    label: &'static str,
    disabled: impl Fn() -> bool + 'static,
    press: impl Fn() + 'static,
) -> impl IntoElement {
    nsh_base::button()
        .on_press(press)
        .disabled(disabled)
        .accessibility_label(label)
        .w(px(14.5))
        .h(px(18.))
        .p_0()
        .items_center()
        .justify_center()
        .hover(|s| s.bg(move || theme().accent))
        .active(|s| s.bg(move || theme().muted))
        .disabled_style(|s| s.opacity(0.35))
        .focus_visible(|s| s.outline(px(2.)).outline_color(move || theme().ring))
        .child(icons::icon(name).size(px(16.)))
}
