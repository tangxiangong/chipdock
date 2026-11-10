use chipdock_gpui::{backend, model, serial, update};
mod assets;
mod ui;
use gpui_kit::*;

fn main() {
    if let Some(result) = chipdock_gpui::platform::handle_driver_install() {
        if let Err(error) = result {
            eprintln!("{error:#}");
            std::process::exit(1);
        }
        return;
    }
    match update::finish_update(&std::env::args().skip(1).collect::<Vec<_>>()) {
        Ok(true) => return,
        Ok(false) => {}
        Err(error) => {
            eprintln!("更新失败: {error:#}");
            std::process::exit(1);
        }
    }
    let firmware = chipdock_gpui::instance::firmware_argument();
    let _instance = match chipdock_gpui::instance::Instance::acquire(
        std::env::args().any(|s| s == "--after-update"),
        firmware.as_deref(),
    ) {
        Ok(Some(instance)) => instance,
        Ok(None) => return,
        Err(error) => {
            eprintln!("无法启动 ChipDock: {error:#}");
            std::process::exit(1);
        }
    };
    gpui_kit::application()
        .with_assets(assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            cx.set_menus([
                Menu::new("ChipDock").items([
                    MenuItem::action("关于 ChipDock", ui::ShowSettings),
                    MenuItem::action("设置…", ui::ShowSettings),
                    MenuItem::separator(),
                    MenuItem::action("退出 ChipDock", ui::Quit),
                ]),
                Menu::new("文件").items([
                    MenuItem::action("打开固件…", ui::OpenFirmware),
                    MenuItem::action("存储当前配置…", ui::SaveProfile),
                    MenuItem::action("保存设置", ui::SaveSettings),
                    MenuItem::action("关闭窗口", ui::CloseWindow),
                ]),
                Menu::new("编辑").items([MenuItem::action("查找…", ui::Find)]),
                Menu::new("显示").items([
                    MenuItem::action("固件烧录", ui::ShowFlash),
                    MenuItem::action("串口连接", ui::ShowSerial),
                    MenuItem::separator(),
                    MenuItem::action("内存", ui::ShowMemory),
                    MenuItem::action("日志", ui::ShowLogs),
                ]),
                Menu::new("目标").items([
                    MenuItem::action("刷新探针／串口", ui::Refresh),
                    MenuItem::action("连接", ui::ConnectTarget),
                    MenuItem::action("断开", ui::DisconnectTarget),
                ]),
                Menu::new("操作").items([
                    MenuItem::action("烧录／发送", ui::ExecutePrimary),
                    MenuItem::action("读取内存", ui::ReadMemory),
                    MenuItem::action("清空终端", ui::ClearTerminal),
                    MenuItem::action("开始／停止自动发送", ui::ToggleAutoSend),
                ]),
            ]);
            let options = WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("ChipDock".into()),
                    traffic_light_position: Some(point(px(19.), px(19.))),
                    ..gpui_kit::component::TitleBar::title_bar_options()
                }),
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(900.), px(850.)),
                    cx,
                ))),
                window_min_size: Some(size(px(900.), px(600.))),
                ..gpui_kit::component::TitleBar::window_options()
            };
            if let Err(error) = gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| ui::Workbench::new(window, cx))
            }) {
                eprintln!("无法打开 ChipDock: {error:#}");
                cx.quit();
                return;
            }
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    if let Err(error) = chipdock_service::serial::serial_shutdown() {
                        eprintln!("{error}");
                    }
                    cx.quit();
                }
            })
            .detach();
            cx.activate(true);
        });
}
