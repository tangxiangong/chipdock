use super::*;
use chipdock_core::models::{FirmwareFormat, MemoryRegionKind};
use nsh_base::{alert_dialog, resizable_panel, v_resizable};

pub(super) fn page(store: Store) -> View {
    let setup = store.target_setup;
    view()
        .flex_1()
        .min_h_0()
        .child(
            v_resizable()
                .flex_1()
                .handle_class("bg-border")
                .handle_accessibility_label("调整固件文档和烧录选项高度")
                .child(resizable_panel().min_size(px(260.)).child(document(store)))
                .child(
                    resizable_panel()
                        .size(px(300.))
                        .min_size(px(260.))
                        .max_size(px(420.))
                        .child(settings(store, setup)),
                ),
        )
        .child(when(setup, move || super::target::dialog(store, setup)))
}
fn margin() -> Points {
    px((nsh_core::element::window().width.0 * 0.05).max(32.))
}
fn document(store: Store) -> View {
    let empty = move || store.field("firmware").is_empty();
    view()
        .flex_1()
        .min_h_0()
        .px(margin)
        .py_6()
        .gap(px(20.))
        .child(
            when(empty, move || empty_document(store)).otherwise(move || firmware_document(store)),
        )
}
fn empty_document(store: Store) -> View {
    let border_size = signal(Size {
        width: px(0.),
        height: px(0.),
    });
    view()
        .flex_1()
        .gap(px(22.))
        .child(
            view()
                .relative()
                .on_layout(move |bounds: &Bounds<Points>| border_size.set(bounds.size))
                .flex_1()
                .min_h(px(170.))
                .py_6()
                .gap_2()
                .items_center()
                .justify_center()
                .rounded(px(10.))
                .child(
                    canvas(move || {
                        let size = border_size.get();
                        let inset = 0.75;
                        Painter::new()
                            .stroke_color(color(theme().foreground.opacity(0.175)))
                            .line_width(px(1.5))
                            .line_dash(&[px(6.), px(4.)])
                            .begin_path()
                            .round_rect(
                                Bounds {
                                    origin: Point {
                                        x: px(inset),
                                        y: px(inset),
                                    },
                                    size: Size {
                                        width: px((size.width.0 - 2. * inset).max(0.)),
                                        height: px((size.height.0 - 2. * inset).max(0.)),
                                    },
                                },
                                px(9.25),
                            )
                            .stroke()
                    })
                    .absolute()
                    .inset_0()
                    .pointer_events(nsh_core::PointerEvents::None),
                )
                .child(
                    view()
                        .size(px(36.))
                        .mb_1()
                        .text_color(move || theme().muted_foreground)
                        .child(icons::icon("drop-doc").size(px(36.))),
                )
                .child(
                    text("将固件拖到这里")
                        .text_size(px(15.))
                        .font_weight(FontWeight::MEDIUM),
                )
                .child(
                    text("支持 ELF、AXF、HEX 与 BIN 文件")
                        .text_size(caption_size)
                        .text_color(move || theme().muted_foreground),
                )
                .child(view().mt_2().child(labeled_button(
                    "folder",
                    "选择文件…",
                    false,
                    move || store.locked("firmware"),
                    move || store.pick("firmware"),
                ))),
        )
        .child(super::firmware_history::page(store))
}
fn firmware_document(store: Store) -> View {
    let preview = computed(move || {
        store.read(|s| {
            s.preview
                .as_ref()
                .map(|p| (p.start_address, p.bytes.clone()))
        })
    });
    let rows = computed(move || {
        preview.with(|p| {
            p.as_ref()
                .map(|(start, bytes)| {
                    bytes
                        .chunks(16)
                        .enumerate()
                        .map(|(i, bytes)| {
                            (
                                format!("0x{:08X}", start + (i * 16) as u64),
                                bytes
                                    .iter()
                                    .map(|b| b.map_or("··".into(), |b| format!("{b:02X}")))
                                    .collect::<Vec<_>>()
                                    .join(" "),
                                bytes
                                    .iter()
                                    .map(|b| {
                                        b.filter(|b| (32..127).contains(b)).map_or('·', char::from)
                                    })
                                    .collect::<String>(),
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
    });
    view()
        .flex_1()
        .min_h_0()
        .gap(px(20.))
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(px(14.))
                .child(
                    view()
                        .size(px(44.))
                        .rounded(px(10.))
                        .items_center()
                        .justify_center()
                        .bg(move || document_tint(store).opacity(0.14))
                        .text_color(move || document_tint(store))
                        .child(icons::icon("file").size(px(22.))),
                )
                .child(
                    view()
                        .flex_1()
                        .min_w_0()
                        .gap(px(3.))
                        .child(
                            text(move || {
                                std::path::Path::new(&store.field("firmware"))
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .into_owned()
                            })
                            .text_size(px(17.))
                            .font_weight(FontWeight::SEMIBOLD),
                        )
                        .child(caption(move || {
                            abbreviate(
                                std::path::Path::new(&store.field("firmware"))
                                    .parent()
                                    .unwrap_or_else(|| std::path::Path::new("")),
                            )
                        })),
                )
                .child(firmware_menu(store)),
        )
        .child(
            when(
                move || preview.with(Option::is_some),
                move || {
                    view()
                        .flex_1()
                        .min_h_0()
                        .gap(px(20.))
                        .child(layout_bar(store))
                        .child(
                            view()
                                .flex_1()
                                .min_h(px(64.))
                                .gap_2()
                                .child(
                                    view()
                                        .flex_row()
                                        .items_center()
                                        .child(text("固件内容").text_size(caption_size))
                                        .child(view().flex_1())
                                        .child(
                                            text(move || {
                                                preview.with(|p| {
                                                    p.as_ref()
                                                        .map(|(start, b)| {
                                                            format!(
                                                                "0x{start:08X} – 0x{:08X}",
                                                                start
                                                                    + b.len().saturating_sub(1)
                                                                        as u64
                                                            )
                                                        })
                                                        .unwrap_or_default()
                                                })
                                            })
                                            .text_size(small_size)
                                            .font_family(mono)
                                            .text_color(move || theme().muted_foreground),
                                        ),
                                )
                                .child(
                                    view()
                                        .flex_1()
                                        .min_h_0()
                                        .border_1()
                                        .border_color(hairline)
                                        .rounded(move || theme().radius)
                                        .child(
                                            view()
                                                .h(px(25.))
                                                .px(px(11.))
                                                .flex_row()
                                                .items_center()
                                                .gap_3()
                                                .text_color(move || theme().muted_foreground)
                                                .bg(move || {
                                                    theme().color("well").unwrap_or(theme().muted)
                                                })
                                                .border_b_1()
                                                .border_color(hairline)
                                                .child(
                                                    text("地址").w(px(80.)).text_size(small_size),
                                                )
                                                .child(
                                                    text("十六进制")
                                                        .w(px(376.))
                                                        .text_size(small_size),
                                                )
                                                .child(text("ASCII").text_size(small_size)),
                                        )
                                        .child(
                                            virtual_list(
                                                move || rows.with(Vec::len),
                                                move |at| {
                                                    view()
                                                        .h(px(17.))
                                                        .px(px(11.))
                                                        .flex_row()
                                                        .items_center()
                                                        .gap_3()
                                                        .font_family(mono)
                                                        .text_size(caption_size)
                                                        .child(
                                                            text(move || {
                                                                rows.with(|r| {
                                                                    r.get(at.index())
                                                                        .map(|r| r.0.clone())
                                                                        .unwrap_or_default()
                                                                })
                                                            })
                                                            .w(px(80.))
                                                            .text_color(move || {
                                                                theme().muted_foreground
                                                            }),
                                                        )
                                                        .child(
                                                            text(move || {
                                                                rows.with(|r| {
                                                                    r.get(at.index())
                                                                        .map(|r| r.1.clone())
                                                                        .unwrap_or_default()
                                                                })
                                                            })
                                                            .w(px(376.)),
                                                        )
                                                        .child(
                                                            text(move || {
                                                                rows.with(|r| {
                                                                    r.get(at.index())
                                                                        .map(|r| r.2.clone())
                                                                        .unwrap_or_default()
                                                                })
                                                            })
                                                            .text_color(move || {
                                                                theme().muted_foreground
                                                            }),
                                                        )
                                                },
                                            )
                                            .estimated_item_size(px(17.))
                                            .content_padding_top(px(3.))
                                            .flex_1(),
                                        ),
                                ),
                        )
                        .child(
                            view()
                                .flex_row()
                                .items_start()
                                .gap(px(36.))
                                .flex_wrap()
                                .children(
                                    [("格式", 0), ("写入大小", 1), ("区段", 2), ("地址范围", 3)]
                                        .into_iter()
                                        .map(move |(label, index)| {
                                            view()
                                            .gap(px(3.))
                                            .child(
                                                text(label)
                                                    .text_size(small_size)
                                                    .text_color(move || theme().muted_foreground),
                                            )
                                            .child(
                                                text(move || {
                                                    store.read(|s| {
                                                        s.preview
                                                            .as_ref()
                                                            .map(|p| {
                                                                let u = &p.usage;
                                                                match index {
                                                                    0 => match u.format {
                                                                        FirmwareFormat::Elf => {
                                                                            "ELF"
                                                                        }
                                                                        FirmwareFormat::Hex => {
                                                                            "HEX"
                                                                        }
                                                                        FirmwareFormat::Bin => {
                                                                            "BIN"
                                                                        }
                                                                    }
                                                                    .into(),
                                                                    1 => byte_count(u.used_bytes),
                                                                    2 => {
                                                                        u.segments.len().to_string()
                                                                    }
                                                                    _ => format!(
                                                                        "0x{:08X} – 0x{:08X}",
                                                                        u.start_address
                                                                            .unwrap_or(0),
                                                                        u.end_address
                                                                            .unwrap_or(1)
                                                                            .saturating_sub(1)
                                                                    ),
                                                                }
                                                            })
                                                            .unwrap_or_default()
                                                    })
                                                })
                                                .text_size(body_size)
                                                .when(index == 3, |text| text.font_family(mono)),
                                            )
                                        }),
                                ),
                        )
                },
            )
            .otherwise(move || {
                view().child(
                    when(
                        move || store.read(|state| state.firmware_error.is_some()),
                        move || {
                            view()
                                .p(px(10.))
                                .rounded(px(6.))
                                .flex_row()
                                .items_start()
                                .gap_2()
                                .selectable(true)
                                .bg(move || {
                                    theme()
                                        .color("firmware-tint")
                                        .unwrap_or(theme().primary)
                                        .opacity(0.08)
                                })
                                .child(icons::icon("warning-fill").size(px(14.)).text_color(
                                    move || {
                                        theme().color("firmware-tint").unwrap_or(theme().primary)
                                    },
                                ))
                                .child(
                                    text(move || {
                                        store.read(|state| {
                                            state.firmware_error.clone().unwrap_or_default()
                                        })
                                    })
                                    .text_size(caption_size)
                                    .flex_1()
                                    .min_w_0(),
                                )
                        },
                    )
                    .otherwise(move || {
                        view().child(when(
                            move || store.read(Workbench::firmware_analyzing),
                            || caption("正在分析固件…"),
                        ))
                    }),
                )
            }),
        )
}
fn firmware_menu(store: Store) -> impl IntoElement {
    use nsh_core::element::desktop::MenuItem;
    view()
        .size(px(24.))
        .items_end()
        .justify_center()
        .child(native_borderless_menu_disabled(
            view()
                .w(px(21.))
                .h(px(15.))
                .items_center()
                .justify_center()
                .label("固件操作")
                .child(icons::icon("more")),
            move || store.locked("firmware"),
            move || {
                vec![
                    MenuItem::new("更换固件…").on_select(move || store.pick("firmware")),
                    MenuItem::new(if cfg!(target_os = "macos") {
                        "在访达中显示"
                    } else {
                        "在文件夹中显示"
                    })
                    .on_select(move || {
                        let path = store.field("firmware");
                        store.spawn(async move {
                            if let Err(error) = reveal_path(path).await {
                                store.report(error);
                            }
                        });
                    }),
                    MenuItem::separator(),
                    MenuItem::new("关闭固件").on_select(move || {
                        store.change(|state| state.set("firmware", String::new()))
                    }),
                ]
            },
        ))
}

fn layout_bar(store: Store) -> View {
    let extent = computed(move || {
        store.read(|s| {
            let Some(p) = &s.preview else {
                return (0, 1, 0, 0, vec![]);
            };
            let u = &p.usage;
            let used_start = u.start_address.unwrap_or(0);
            let used_end = u.end_address.unwrap_or(used_start);
            let flash = s
                .layout
                .iter()
                .filter(|r| matches!(r.kind, MemoryRegionKind::Nvm) && !r.is_alias)
                .filter(|r| r.start < used_end && r.end > used_start)
                .collect::<Vec<_>>();
            let start = flash
                .iter()
                .map(|r| r.start)
                .min()
                .unwrap_or(used_start)
                .min(used_start);
            let end = flash
                .iter()
                .map(|r| r.end)
                .max()
                .unwrap_or(used_end)
                .max(used_end.max(1));
            (
                start,
                end,
                flash.iter().map(|r| r.size).sum::<u64>(),
                u.used_bytes,
                u.segments
                    .iter()
                    .map(|s| (s.start, s.size))
                    .collect::<Vec<_>>(),
            )
        })
    });
    view()
        .gap(px(6.))
        .child(
            view()
                .relative()
                .h(px(8.))
                .rounded(px(3.))
                .overflow_hidden()
                .bg(move || theme().color("well").unwrap_or(theme().muted))
                .child(each(
                    computed(move || extent.with(|e| e.4.clone())),
                    |r: &(u64, u64)| r.0,
                    move |segment, _| {
                        view()
                            .absolute()
                            .h_full()
                            .left(move || {
                                percentage(extent.with(|e| {
                                    (segment.get().0 - e.0) as f32 / (e.1 - e.0).max(1) as f32
                                }))
                            })
                            .w(move || {
                                percentage(
                                    extent.with(|e| {
                                        segment.get().1 as f32 / (e.1 - e.0).max(1) as f32
                                    }),
                                )
                            })
                            .min_w(px(2.))
                            .bg(move || theme().primary)
                    },
                )),
        )
        .child(
            view()
                .flex_row()
                .items_center()
                .font_family(mono)
                .text_size(small_size)
                .text_color(move || theme().muted_foreground)
                .child(text(move || extent.with(|e| format!("0x{:08X}", e.0))))
                .child(view().flex_1())
                .child(text(move || {
                    extent.with(|e| {
                        if e.2 > 0 {
                            format!(
                                "Flash {} · 已用 {:.1}%",
                                byte_count(e.2),
                                e.3 as f64 / e.2 as f64 * 100.
                            )
                        } else {
                            String::new()
                        }
                    })
                }))
                .child(view().flex_1())
                .child(text(move || {
                    extent.with(|e| format!("0x{:08X}", e.1.saturating_sub(1)))
                })),
        )
}
fn settings(store: Store, setup: Signal<bool>) -> impl IntoElement {
    let label = |s: &'static str| {
        text(s)
            .w(px(52.))
            .text_right()
            .text_size(body_size)
            .text_color(move || theme().muted_foreground)
    };
    scroll().flex_1().child(
        view()
            .px(margin)
            .py_6()
            .gap_6()
            .child(
                view()
                    .gap_4()
                    .child(
                        view()
                            .flex_row()
                            .items_start()
                            .gap(px(14.))
                            .child(label("目标").mt_1())
                            .child(
                                view()
                                    .flex_1()
                                    .mt_1()
                                    .gap(px(3.))
                                    .child(
                                        view()
                                            .flex_row()
                                            .items_center()
                                            .gap(px(6.))
                                            .child(view().size(px(7.)).rounded_full().bg(
                                                move || {
                                                    store.read(|s| {
                                                        if s.target_connected() {
                                                            theme()
                                                                .color("success")
                                                                .unwrap_or(theme().primary)
                                                        } else {
                                                            theme().muted_foreground.opacity(0.4)
                                                        }
                                                    })
                                                },
                                            ))
                                            .child(
                                                text(move || store.read(Workbench::target_name))
                                                    .text_size(body_size)
                                                    .font_weight(FontWeight::MEDIUM),
                                            )
                                            .child(when(
                                                move || !store.read(Workbench::target_connected),
                                                || caption("未连接"),
                                            )),
                                    )
                                    .child(caption(move || {
                                        format!(
                                            "{} · {} kHz · {}",
                                            store.field("wire").to_uppercase(),
                                            store.field("speed"),
                                            if store.field("probe").is_empty() {
                                                "自动选择探针".into()
                                            } else {
                                                store.field("probe")
                                            }
                                        )
                                    })),
                            )
                            .child(when(
                                move || store.read(Workbench::target_connected),
                                move || {
                                    labeled_button(
                                        "close",
                                        "断开连接",
                                        false,
                                        move || store.read(|s| s.lane_busy(Lane::Probe)),
                                        move || store.change(Workbench::disconnect_target),
                                    )
                                },
                            ))
                            .child(labeled_button(
                                "settings",
                                "更改…",
                                false,
                                move || store.read(|s| s.lane_busy(Lane::Probe)),
                                move || setup.set(true),
                            )),
                    )
                    .child(
                        view()
                            .flex_row()
                            .items_start()
                            .gap(px(14.))
                            .child(label("擦除").mt_1())
                            .child(
                                view()
                                    .gap_1()
                                    .child(view().w(px(100.)).child(popup_control(
                                        store,
                                        "erase",
                                        &["automatic", "full", "skip"],
                                        false,
                                    )))
                                    .child(caption(move || match store.field("erase").as_str() {
                                        "full" => "写入前擦除整个 Flash，其他数据会丢失",
                                        "skip" => "不擦除，目标区域须已擦除",
                                        _ => "仅擦除固件覆盖的扇区",
                                    })),
                            ),
                    )
                    .child(
                        view()
                            .flex_row()
                            .items_start()
                            .gap(px(14.))
                            .child(label("完成后"))
                            .child(
                                view()
                                    .gap(px(7.))
                                    .child(toggle(store, "verify", "校验写入的数据"))
                                    .child(toggle(store, "reset", "复位并运行程序"))
                                    .child(toggle(store, "dry_run", "仅预检查，不写入芯片")),
                            ),
                    ),
            )
            .child(
                view()
                    .flex_row()
                    .items_center()
                    .gap_4()
                    .child(caption(move || {
                        store.read(|s| -> String {
                            if s.lane_busy(Lane::Probe) {
                                "".into()
                            } else if !s.target_connected() {
                                "连接后即可烧录（⌘K）".into()
                            } else if s.preferences.get("firmware").is_empty() {
                                "选择固件后即可烧录".into()
                            } else {
                                "按 ⌘↩ 开始".into()
                            }
                        })
                    }))
                    .child(view().flex_1())
                    .child(labeled_button(
                        "erase",
                        "擦除芯片…",
                        false,
                        move || store.read(|s| !s.target_connected() || s.lane_busy(Lane::Probe)),
                        move || confirm_erase(store),
                    ))
                    .child(labeled_button(
                        "link",
                        move || {
                            store.read(|s| {
                                if s.lane_busy(Lane::Probe) {
                                    "正在处理…".into()
                                } else if !s.target_connected() {
                                    "连接目标".into()
                                } else if s.preferences.flag("dry_run") {
                                    "开始预检查".into()
                                } else {
                                    format!("烧录到 {}", s.preferences.get("chip"))
                                }
                            })
                        },
                        true,
                        move || {
                            store.read(|s| {
                                s.lane_busy(Lane::Probe)
                                    || (s.target_connected()
                                        && s.preferences.get("firmware").is_empty())
                            })
                        },
                        move || {
                            if store.read(Workbench::target_connected) {
                                execute(store, false)
                            } else {
                                store.command(|s| Command::Connect(s.preferences.clone()))
                            }
                        },
                    )),
            ),
    )
}
fn abbreviate(path: &std::path::Path) -> String {
    let path = path.to_string_lossy();
    if let Ok(home) = std::env::var("HOME")
        && let Some(suffix) = path.strip_prefix(&home)
    {
        format!("~{suffix}")
    } else {
        path.into_owned()
    }
}
fn byte_count(bytes: u64) -> String {
    if bytes >= 1048576 {
        format!("{:.1} MiB", bytes as f64 / 1048576.)
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.)
    } else {
        format!("{bytes} B")
    }
}
pub(super) fn execute(store: Store, preflight: bool) {
    if store.read(|s| {
        !s.target_connected()
            || s.lane_busy(Lane::Probe)
            || s.preferences.get("firmware").is_empty()
    }) {
        return;
    }
    let mut p = store.read(|s| s.preferences.clone());
    if preflight {
        p.flags.insert("dry_run".into(), true);
    }
    if p.get("erase") == "full" && !p.flag("dry_run") {
        store.spawn(async move {
            let result = alert_dialog()
                .title(format!("擦除并烧录 {}？", p.get("chip")))
                .description("整片擦除会永久删除目标上的全部固件和数据。")
                .cancel("cancel", "取消")
                .destructive("flash", "擦除并烧录")
                .show()
                .await;
            if matches!(result,Ok(r) if r.action.as_deref()==Some("flash"))
                && store
                    .read(|s| s.target_connected() && s.preferences.target_key() == p.target_key())
            {
                store.command(|_| Command::Flash(p));
            }
        });
    } else {
        store.command(|_| Command::Flash(p));
    }
}
pub(super) fn confirm_erase(store: Store) {
    if store.read(|state| !state.target_connected() || state.lane_busy(Lane::Probe)) {
        return;
    }
    let p = store.read(|s| s.preferences.clone());
    store.spawn(async move {
        let result = alert_dialog()
            .title(format!("擦除 {}？", p.get("chip")))
            .description("目标的全部固件和数据将被永久删除。")
            .cancel("cancel", "取消")
            .destructive("erase", "擦除")
            .show()
            .await;
        if matches!(result,Ok(r) if r.action.as_deref()==Some("erase"))
            && store.read(|s| s.target_connected() && s.preferences.target_key() == p.target_key())
        {
            store.command(|_| Command::Erase(p));
        }
    });
}

fn document_tint(store: Store) -> AdaptiveColor {
    store.read(|state| {
        state
            .preview
            .as_ref()
            .and_then(|preview| {
                let role = match preview.usage.format {
                    FirmwareFormat::Elf => "firmware-elf",
                    FirmwareFormat::Hex => "firmware-tint",
                    FirmwareFormat::Bin => "firmware-bin",
                };
                theme().color(role)
            })
            .unwrap_or(theme().muted_foreground)
    })
}
