//! Native file-panel presentation retained from the macOS reference client.
//!
//! NSH's public picker attaches visible-window open/save panels as sheets and
//! maps its title to a message. The reference deliberately uses both standalone
//! AppKit `begin` panels and SwiftUI document sheets, so this narrow bridge
//! keeps that distinction. The caller owns the file-dialog reentry guard.

use std::path::PathBuf;

#[derive(Clone, Copy)]
pub(super) enum ExportKind {
    Save,
    DocumentData,
    DocumentText,
}

pub(super) async fn save_path(name: String, kind: ExportKind) -> Result<Option<PathBuf>, String> {
    #[cfg(target_os = "macos")]
    {
        macos_save_path(name, kind).await
    }
    #[cfg(not(target_os = "macos"))]
    {
        use nsh_core::file_dialog::{FileFilter, SaveDialog};
        let dialog = SaveDialog::new().file_name(name);
        let dialog = match kind {
            ExportKind::Save => dialog,
            ExportKind::DocumentData => dialog.button_label("导出"),
            ExportKind::DocumentText => dialog
                .button_label("导出")
                .filter(FileFilter::new("文本", ["txt"])),
        };
        nsh_core::element::pick_save_path(dialog).await
    }
}

#[cfg(target_os = "macos")]
async fn macos_save_path(name: String, kind: ExportKind) -> Result<Option<PathBuf>, String> {
    let document_dark = nsh_core::element::dark();
    let (answer, result) = tokio::sync::oneshot::channel();
    nsh_core::element::run_on_main_thread(move || -> Result<(), String> {
        use block2::RcBlock;
        use nsh_platform_macos::{
            objc2::MainThreadMarker,
            objc2_app_kit::{
                NSAppearance, NSAppearanceCustomization, NSAppearanceNameAqua,
                NSAppearanceNameDarkAqua, NSApplication, NSSavePanel,
            },
            objc2_foundation::{NSArray, NSString},
        };
        use objc2_uniform_type_identifiers::{UTTypeData, UTTypePlainText};
        use std::cell::RefCell;

        let marker =
            MainThreadMarker::new().ok_or_else(|| "无法在主线程打开导出窗口".to_owned())?;
        let app = NSApplication::sharedApplication(marker);
        let parent = match kind {
            ExportKind::Save => None,
            ExportKind::DocumentData | ExportKind::DocumentText => Some(
                app.keyWindow()
                    .ok_or_else(|| "导出来源窗口已关闭".to_owned())?,
            ),
        };
        let panel = NSSavePanel::savePanel(marker);
        panel.setNameFieldStringValue(&NSString::from_str(&name));
        if !matches!(kind, ExportKind::Save) {
            // SwiftUI document export follows its source window's chosen
            // appearance. Standalone native save panels retain the system's.
            // SAFETY: The appearance names are immutable AppKit constants.
            let appearance_name = unsafe {
                if document_dark {
                    NSAppearanceNameDarkAqua
                } else {
                    NSAppearanceNameAqua
                }
            };
            let appearance = NSAppearance::appearanceNamed(appearance_name);
            panel.setAppearance(appearance.as_deref());
            panel.setTitle(Some(&NSString::from_str("导出")));
            panel.setNameFieldLabel(Some(&NSString::from_str("导出为：")));
            panel.setPrompt(Some(&NSString::from_str("导出")));
            panel.setShowsTagField(false);
            // SAFETY: These immutable framework constants are valid for the
            // process lifetime. AppKit owns the native format popup, including
            // its "其他格式" flow; there is no app-drawn accessory to maintain.
            let (data, text) = unsafe { (UTTypeData, UTTypePlainText) };
            panel.setAllowedContentTypes(&NSArray::from_slice(&[data, text]));
            panel.setAllowsOtherFileTypes(true);
            panel.setCurrentContentType(Some(if matches!(kind, ExportKind::DocumentText) {
                text
            } else {
                data
            }));
            panel.setShowsContentTypes(matches!(kind, ExportKind::DocumentData));
        }

        let chosen = panel.clone();
        let answer = RefCell::new(Some(answer));
        let completion = RcBlock::new(move |response: isize| {
            let picked = if response == 1 {
                chosen
                    .URL()
                    .and_then(|url| url.path())
                    .map(|path| PathBuf::from(path.to_string()))
                    .map(Some)
                    .ok_or_else(|| "导出窗口未返回本地文件路径".to_owned())
            } else if response == 0 {
                Ok(None)
            } else {
                Err(format!("导出窗口未能完成（{response}）"))
            };
            chosen.orderOut(None);
            if let Some(answer) = answer.borrow_mut().take() {
                let _ = answer.send(picked);
            }
        });
        app.activate();
        if let Some(parent) = parent {
            panel.beginSheetModalForWindow_completionHandler(&parent, &completion);
        } else {
            panel.beginWithCompletionHandler(&completion);
        }
        Ok(())
    })
    .await
    .map_err(|error| format!("无法打开导出窗口：{error}"))??;
    result.await.map_err(|_| "导出窗口已关闭".to_owned())?
}

#[derive(Clone, Copy)]
pub(super) enum OpenKind {
    File,
    Json,
    DocumentJson,
    DocumentFirmware,
    Folder,
}

pub(super) async fn open_path(kind: OpenKind) -> Result<Option<PathBuf>, String> {
    #[cfg(target_os = "macos")]
    {
        macos_open_path(kind).await
    }
    #[cfg(not(target_os = "macos"))]
    {
        use nsh_core::file_dialog::{FileFilter, OpenDialog};
        let dialog = match kind {
            OpenKind::File => OpenDialog::files(),
            OpenKind::Json | OpenKind::DocumentJson => {
                OpenDialog::files().filter(FileFilter::new("JSON", ["json"]))
            }
            OpenKind::DocumentFirmware => OpenDialog::files()
                .filter(FileFilter::new("固件文件", ["elf", "axf", "hex", "bin"])),
            OpenKind::Folder => OpenDialog::folders(),
        };
        Ok(nsh_core::element::pick_paths(dialog)
            .await?
            .and_then(|paths| paths.into_iter().next()))
    }
}

#[cfg(target_os = "macos")]
async fn macos_open_path(kind: OpenKind) -> Result<Option<PathBuf>, String> {
    let document_dark = nsh_core::element::dark();
    let (answer, result) = tokio::sync::oneshot::channel();
    nsh_core::element::run_on_main_thread(move || -> Result<(), String> {
        use block2::RcBlock;
        use nsh_platform_macos::{
            objc2::MainThreadMarker,
            objc2_app_kit::{
                NSAppearance, NSAppearanceCustomization, NSAppearanceNameAqua,
                NSAppearanceNameDarkAqua, NSApplication, NSOpenPanel,
            },
            objc2_foundation::{NSArray, NSString},
        };
        use objc2_uniform_type_identifiers::{UTType, UTTypeJSON};
        use std::cell::RefCell;

        let marker =
            MainThreadMarker::new().ok_or_else(|| "无法在主线程打开文件窗口".to_owned())?;
        let app = NSApplication::sharedApplication(marker);
        let parent = if matches!(kind, OpenKind::DocumentJson | OpenKind::DocumentFirmware) {
            Some(
                app.keyWindow()
                    .ok_or_else(|| "导入来源窗口已关闭".to_owned())?,
            )
        } else {
            None
        };
        let panel = NSOpenPanel::openPanel(marker);
        if parent.is_some() {
            // SAFETY: The appearance names are immutable AppKit constants.
            let appearance_name = unsafe {
                if document_dark {
                    NSAppearanceNameDarkAqua
                } else {
                    NSAppearanceNameAqua
                }
            };
            let appearance = NSAppearance::appearanceNamed(appearance_name);
            panel.setAppearance(appearance.as_deref());
            panel.setCanCreateDirectories(true);
        }
        match kind {
            OpenKind::File => {}
            OpenKind::Json | OpenKind::DocumentJson => {
                // SAFETY: This framework constant is immutable and valid for
                // the process lifetime; AppKit retains its allowed types.
                panel.setAllowedContentTypes(&NSArray::from_slice(&[unsafe { UTTypeJSON }]));
            }
            OpenKind::DocumentFirmware => {
                let types: Vec<_> = ["elf", "axf", "hex", "bin"]
                    .into_iter()
                    .filter_map(|extension| {
                        UTType::typeWithFilenameExtension(&NSString::from_str(extension))
                    })
                    .collect();
                panel.setAllowedContentTypes(&NSArray::from_retained_slice(&types));
            }
            OpenKind::Folder => {
                panel.setCanChooseFiles(false);
                panel.setCanChooseDirectories(true);
                panel.setCanCreateDirectories(true);
            }
        }
        let chosen = panel.clone();
        let answer = RefCell::new(Some(answer));
        let completion = RcBlock::new(move |response: isize| {
            let picked = if response == 1 {
                chosen
                    .URL()
                    .and_then(|url| url.path())
                    .map(|path| PathBuf::from(path.to_string()))
                    .map(Some)
                    .ok_or_else(|| "文件窗口未返回本地路径".to_owned())
            } else if response == 0 {
                Ok(None)
            } else {
                Err(format!("文件窗口未能完成（{response}）"))
            };
            chosen.orderOut(None);
            if let Some(answer) = answer.borrow_mut().take() {
                let _ = answer.send(picked);
            }
        });
        app.activate();
        if let Some(parent) = parent {
            panel.beginSheetModalForWindow_completionHandler(&parent, &completion);
        } else {
            panel.beginWithCompletionHandler(&completion);
        }
        Ok(())
    })
    .await
    .map_err(|error| format!("无法打开文件窗口：{error}"))??;
    result.await.map_err(|_| "文件窗口已关闭".to_owned())?
}
