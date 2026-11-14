//! Register NSH application menus with AppKit's system roles. Selected-value
//! popups retain the NSH control and return only the native chosen option ID.
use dispatch2::DispatchQueue;
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained, sel,
};
use objc2_app_kit::{
    NSAccessibility, NSApplication, NSApplicationDidUpdateNotification, NSButton, NSFont, NSImage,
    NSMenu, NSMenuDidAddItemNotification, NSMenuDidBeginTrackingNotification, NSMenuItem,
    NSPasteboardTypeString, NSResponder, NSTextField, NSTextView, NSView, NSWindowButton,
};
use objc2_foundation::{
    NSNotification, NSNotificationCenter, NSObject, NSObjectProtocol, NSString,
};
use std::{
    cell::{Cell, OnceCell, RefCell},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

struct ModalClose {
    button: Retained<NSButton>,
    enabled: bool,
}

struct ModalItem {
    item: Retained<NSMenuItem>,
    action: Option<objc2::runtime::Sel>,
    enabled: bool,
}

struct PopupFont {
    registration: u64,
    item_count: usize,
    menu: Option<Retained<NSMenu>>,
    font_size: f64,
    minimum_width: f64,
}

/// A simple retained selector option. Values return to the NSH UI owner;
/// no Store, reactive signal or opaque NSH action crosses to AppKit.
pub(super) struct ChoiceOption {
    pub value: usize,
    pub title: String,
    pub enabled: bool,
    pub separator_before: bool,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements. The target is confined
    // to the main-thread menu's tracking lifetime and owns only its chosen ID.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = Cell<Option<usize>>]
    struct ChipDockChoiceMenuTarget;

    // SAFETY: NSObjectProtocol has no additional requirements.
    unsafe impl NSObjectProtocol for ChipDockChoiceMenuTarget {}

    impl ChipDockChoiceMenuTarget {
        // SAFETY: Each item targets this selector with the item as its sender.
        #[unsafe(method(chooseChoice:))]
        fn choose(&self, sender: &NSMenuItem) {
            if let Ok(value) = usize::try_from(sender.tag()) {
                self.ivars().set(Some(value));
            }
        }
    }
);

/// AppKit must receive the current item at the popup call to establish its
/// initial highlight and keyboard position. NSH's generic action popup passes
/// None, so retain its semantic closed control and bridge only this selector.
pub(super) async fn choice_popup(
    options: Vec<ChoiceOption>,
    selected: Option<usize>,
    small: bool,
    minimum_width: f64,
    page: (f64, f64),
) -> Result<Option<usize>, String> {
    if !minimum_width.is_finite()
        || minimum_width < 0.
        || !page.0.is_finite()
        || !page.1.is_finite()
        || options.iter().any(|item| item.value > isize::MAX as usize)
    {
        return Err("选择菜单的位置或选项无效".to_owned());
    }
    if options.is_empty() {
        return Ok(None);
    }
    let (answer, result) = tokio::sync::oneshot::channel();
    nsh_core::element::run_on_main_thread(move || -> Result<(), String> {
        use block2::RcBlock;
        use objc2_foundation::{NSArray, NSPoint, NSRunLoop, NSRunLoopCommonModes};

        let main = MainThreadMarker::new().ok_or_else(|| "无法在主线程显示选择菜单".to_owned())?;
        let application = NSApplication::sharedApplication(main);
        let window = application
            .keyWindow()
            .ok_or_else(|| "选择菜单来源窗口已关闭".to_owned())?;
        let view = window
            .contentView()
            .ok_or_else(|| "选择菜单来源视图已关闭".to_owned())?;
        let menu = NSMenu::initWithTitle(NSMenu::alloc(main), &NSString::new());
        menu.setAutoenablesItems(false);
        menu.setMinimumWidth(minimum_width);
        // SAFETY: This main-thread menu receives AppKit's own valid menu font.
        unsafe {
            menu.setFont(Some(&NSFont::menuFontOfSize(if small { 11. } else { 13. })));
        }
        let target = ChipDockChoiceMenuTarget::alloc(main).set_ivars(Cell::new(None));
        // SAFETY: NSObject's initializer; this target stays alive through tracking.
        let target: Retained<ChipDockChoiceMenuTarget> = unsafe { msg_send![super(target), init] };
        let mut current = None;
        for option in options {
            if option.separator_before && menu.numberOfItems() > 0 {
                menu.addItem(&NSMenuItem::separatorItem(main));
            }
            // SAFETY: The retained target implements this sender action.
            let item = unsafe {
                NSMenuItem::initWithTitle_action_keyEquivalent(
                    NSMenuItem::alloc(main),
                    &NSString::from_str(&option.title),
                    Some(sel!(chooseChoice:)),
                    &NSString::new(),
                )
            };
            item.setTag(option.value as isize);
            item.setEnabled(option.enabled);
            // SAFETY: The target is held until the menu's synchronous tracking ends.
            unsafe {
                item.setTarget(Some(&target));
            }
            if selected == Some(option.value) {
                item.setState(1);
                current = Some(item.clone());
            }
            menu.addItem(&item);
        }
        // NSH's page coordinates have a top-left origin; the public content
        // container may be unflipped even though its full-size NSH child is not.
        let bounds = view.bounds();
        let location = NSPoint::new(
            bounds.origin.x + page.0,
            if view.isFlipped() {
                bounds.origin.y + page.1
            } else {
                bounds.origin.y + bounds.size.height - page.1
            },
        );
        let answer = RefCell::new(Some(answer));
        let block = RcBlock::new(move || {
            // The native menu loop starts after NSH has committed this event,
            // outside the main-queue drain, just like NSH's own popup scheduler.
            if window.isVisible() {
                menu.popUpMenuPositioningItem_atLocation_inView(
                    current.as_deref(),
                    location,
                    Some(&view),
                );
            }
            if let Some(answer) = answer.borrow_mut().take() {
                let _ = answer.send(target.ivars().get());
            }
        });
        // SAFETY: The immutable mode constant and retained block belong to this
        // main run loop; the block does not access any reactive application state.
        let modes = NSArray::from_slice(&[unsafe { NSRunLoopCommonModes }]);
        unsafe { NSRunLoop::mainRunLoop().performInModes_block(&modes, &block) };
        Ok(())
    })
    .await
    .map_err(|error| error.to_string())??;
    result.await.map_err(|_| "选择菜单已关闭".to_owned())
}

pub(super) struct PopupFontGuard(u64);

impl PopupFontGuard {
    pub(super) async fn with_min_width(
        item_count: usize,
        small: bool,
        minimum_width: f64,
    ) -> Result<Self, String> {
        if !minimum_width.is_finite() || minimum_width < 0. {
            return Err("菜单最小宽度无效".to_owned());
        }
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let registration = NEXT.fetch_add(1, Ordering::Relaxed);
        nsh_core::element::run_on_main_thread(move || {
            MainThreadMarker::new().ok_or_else(|| "无法在主线程设置菜单字体".to_owned())?;
            POPUP_FONT.with(|scope| {
                *scope.borrow_mut() = Some(PopupFont {
                    registration,
                    item_count,
                    menu: None,
                    font_size: if small { 11. } else { 13. },
                    minimum_width,
                })
            });
            Ok::<_, String>(())
        })
        .await
        .map_err(|error| error.to_string())??;
        Ok(Self(registration))
    }
}

impl Drop for PopupFontGuard {
    fn drop(&mut self) {
        let registration = self.0;
        DispatchQueue::main().exec_async(move || {
            POPUP_FONT.with(|scope| {
                let mut scope = scope.borrow_mut();
                if scope
                    .as_ref()
                    .is_some_and(|scope| scope.registration == registration)
                {
                    *scope = None;
                }
            });
        });
    }
}

static MAIN_MODAL: AtomicBool = AtomicBool::new(false);

define_class!(
    // SAFETY: NSObject has no subclassing requirements. The observer owns no
    // AppKit views and runs only on the application's main thread.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    struct SystemMenuObserver;

    // SAFETY: NSObjectProtocol has no additional requirements.
    unsafe impl NSObjectProtocol for SystemMenuObserver {}

    impl SystemMenuObserver {
        // SAFETY: NotificationCenter sends the declared notification argument.
        #[unsafe(method(updateMenus:))]
        fn update_menus(&self, notification: &NSNotification) {
            apply_popup_font(notification);
            if notification.name().isEqualToString(unsafe { NSApplicationDidUpdateNotification }) {
                label_hosted_inputs(self.mtm());
            }
            // Item-add notifications arrive while NSH is constructing a menu;
            // leave its application graph untouched until AppKit's update.
            if !notification.name().isEqualToString(unsafe {NSMenuDidAddItemNotification}) {
                register_current_menus(self.mtm());
            }

        }
    }
);

thread_local! {
    // The selector observer lives for the app's main-thread lifetime. It does
    // not replace NSH's delegate or retain a second copy of the menu graph.
    static OBSERVER: OnceCell<Retained<SystemMenuObserver>> = const { OnceCell::new() };
    static SETTINGS_COG: OnceCell<Option<Retained<NSImage>>> = const { OnceCell::new() };
    // Only retained while a real modal guard is active; no second menu graph.
    static MODAL_ITEMS: RefCell<Vec<ModalItem>> = const { RefCell::new(Vec::new()) };
    static MODAL_CLOSE: RefCell<Option<ModalClose>> = const { RefCell::new(None) };
    static POPUP_FONT: RefCell<Option<PopupFont>> = const { RefCell::new(None) };
}

/// A projection of NSH's real dialog/file-panel state for AppKit's generated
/// window commands. Application-modal native alerts are checked separately.
pub(super) fn set_main_modal(modal: bool) {
    if MAIN_MODAL.swap(modal, Ordering::AcqRel) != modal {
        DispatchQueue::main().exec_async(|| {
            if let Some(main) = MainThreadMarker::new() {
                register_current_menus(main);
            }
        });
    }
}

pub(super) fn install() {
    DispatchQueue::main().exec_async(|| {
        let Some(main) = MainThreadMarker::new() else {
            return;
        };
        OBSERVER.with(|observer| {
            observer.get_or_init(|| {
                // SAFETY: NSObject's init has this signature, and this class
                // does not add ivars or override initialization.
                let observer: Retained<SystemMenuObserver> =
                    unsafe { msg_send![SystemMenuObserver::alloc(main), init] };
                let application = NSApplication::sharedApplication(main);
                // SAFETY: The retained observer implements updateMenus: with
                // the notification signature; both it and NSApplication are
                // main-thread objects. NotificationCenter does not own it.
                unsafe {
                    NSNotificationCenter::defaultCenter().addObserver_selector_name_object(
                        &observer,
                        sel!(updateMenus:),
                        Some(NSApplicationDidUpdateNotification),
                        Some(&application),
                    );
                    NSNotificationCenter::defaultCenter().addObserver_selector_name_object(
                        &observer,
                        sel!(updateMenus:),
                        Some(NSMenuDidBeginTrackingNotification),
                        None,
                    );
                    NSNotificationCenter::defaultCenter().addObserver_selector_name_object(
                        &observer,
                        sel!(updateMenus:),
                        Some(NSMenuDidAddItemNotification),
                        None,
                    );
                }
                observer
            });
        });
        register_current_menus(main);
    });
}

fn apply_popup_font(notification: &NSNotification) {
    // Application notifications are excluded by the checked downcast; only
    // a live native menu can claim the caller's font scope.
    let Some(menu) = notification
        .object()
        .and_then(|object| object.downcast::<NSMenu>().ok())
    else {
        return;
    };
    // SAFETY: This notification name is an immutable AppKit constant.
    let tracking = notification
        .name()
        .isEqualToString(unsafe { NSMenuDidBeginTrackingNotification });
    let chosen = POPUP_FONT.with(|scope| {
        let mut scope = scope.borrow_mut();
        let scope = scope.as_mut()?;
        if let Some(chosen) = &scope.menu {
            return std::ptr::eq(&**chosen, &*menu)
                .then(|| (chosen.clone(), scope.font_size, scope.minimum_width));
        }
        // NSH names application submenus; a root popup alone has an empty
        // title. Claim once, before layout when its last item is added.
        if menu.title().length() != 0 {
            return None;
        }
        let count = menu.numberOfItems() as usize;
        if count == 0 || (!tracking && count != scope.item_count) || count > scope.item_count {
            return None;
        }
        // SAFETY: NSH keeps the menu graph alive while it is being built/tracked.
        if unsafe { menu.supermenu() }.is_some() {
            return None;
        }
        let target = menu.itemArray().iter().any(|item| {
            item.action() == Some(sel!(menuItemChosen:))
                && item
                    .target()
                    .is_some_and(|target| target.class().name().to_bytes() == b"NshMenuTarget")
        });
        if !target {
            return None;
        }
        scope.menu = Some(menu.clone());
        Some((menu, scope.font_size, scope.minimum_width))
    });
    if let Some((menu, font_size, minimum_width)) = chosen {
        fn set_font_tree(menu: &NSMenu, face: &NSFont) {
            // SAFETY: A real retained font is supplied, never None.
            unsafe {
                menu.setFont(Some(face));
            }
            for item in menu.itemArray().iter() {
                if let Some(submenu) = item.submenu() {
                    set_font_tree(&submenu, face);
                }
            }
            menu.update();
        }
        if minimum_width > 0. {
            menu.setMinimumWidth(minimum_width);
        }
        set_font_tree(&menu, &NSFont::menuFontOfSize(font_size));
    }
}

/// Native hosted editors do not inherit their NSH semantic wrapper's label.
/// Carry only existing public parent labels, preserving any editor-owned name.
fn label_hosted_inputs(main: MainThreadMarker) {
    fn visit(view: &NSView, inherited: Option<&NSString>) {
        let own = NSAccessibility::accessibilityLabel(view);
        let nearest = own
            .as_deref()
            .filter(|label| label.length() > 0)
            .or(inherited);
        let is_input = view
            .downcast_ref::<NSTextField>()
            .is_some_and(|field| field.isEditable() || field.isSelectable())
            || view
                .downcast_ref::<NSTextView>()
                .is_some_and(|field| field.isEditable() || field.isSelectable());
        if is_input
            && own.is_none()
            && let Some(label) = inherited
        {
            NSAccessibility::setAccessibilityLabel(view, Some(label));
        }
        for child in view.subviews().iter() {
            visit(&child, nearest);
        }
    }
    for window in NSApplication::sharedApplication(main).windows().iter() {
        if let Some(content) = window.contentView() {
            visit(&content, None);
        }
    }
}

fn submenu(menu: &NSMenu, label: &str) -> Option<Retained<NSMenu>> {
    menu.itemWithTitle(&NSString::from_str(label))?.submenu()
}

fn different(current: Option<Retained<NSMenu>>, wanted: &NSMenu) -> bool {
    current.is_none_or(|current| !std::ptr::eq(&*current, wanted))
}

fn register_current_menus(main: MainThreadMarker) {
    let application = NSApplication::sharedApplication(main);
    guard_modal_close_button(&application);
    let Some(menu) = application.mainMenu() else {
        return;
    };
    if let Some(app) = submenu(&menu, "ChipDock") {
        // The current native client uses AppKit's standard 199 pt app menu.
        if app.minimumWidth() != 199. {
            app.setMinimumWidth(199.);
        }
        if let Some(settings) = app.itemWithTitle(&NSString::from_str("设置…")) {
            // Use the native Settings symbol. ActionTemplate is an ellipsis
            // on current macOS, and recent AppKit hides images by default.
            if settings.respondsToSelector(sel!(setPreferredImageVisibility:)) {
                // SAFETY: This public NSInteger property exists, as checked.
                let () = unsafe { msg_send![&*settings, setPreferredImageVisibility: 1_isize] };
            }
            let cog = SETTINGS_COG.with(|image| {
                image
                    .get_or_init(|| {
                        NSImage::imageWithSystemSymbolName_accessibilityDescription(
                            &NSString::from_str("gearshape"),
                            None,
                        )
                    })
                    .clone()
            });
            if let Some(cog) = cog
                && settings
                    .image()
                    .as_ref()
                    .is_none_or(|image| !std::ptr::eq(&**image, &*cog))
            {
                settings.setImage(Some(&cog));
            }
        }
    }
    if let Some(file) = submenu(&menu, "文件")
        && let Some(save) = file.itemWithTitle(&NSString::from_str("保存设置"))
    {
        // Keep NSH's global shortcut and its handler while matching the native
        // File menu. A component-local on_key would miss auxiliary windows.
        save.setAllowsKeyEquivalentWhenHidden(true);
        save.setHidden(true);
    }
    if let Some(edit) = submenu(&menu, "编辑")
        && edit.itemWithTitle(&NSString::from_str("删除")).is_none()
    {
        let before_select_all = edit.indexOfItemWithTitle(&NSString::from_str("全选"));
        if before_select_all >= 0 {
            // NSH has no Delete menu role. Keep AppKit's standard nil-target
            // responder dispatch and its existing menu validation; never turn
            // on auto-enabling for the whole menu or override app guards.
            // SAFETY: delete: is the standard Cocoa text-edit action, with a
            // sender argument, resolved and validated by the responder chain.
            let delete = unsafe {
                NSMenuItem::initWithTitle_action_keyEquivalent(
                    NSMenuItem::alloc(main),
                    &NSString::from_str("删除"),
                    Some(sel!(delete:)),
                    &NSString::new(),
                )
            };
            // SAFETY: nil is the standard target for responder-chain actions.
            unsafe {
                delete.setTarget(None);
            }
            edit.insertItem_atIndex(&delete, before_select_all);
        }
    }
    // NSH replaces the native menu graph when visible command state changes.
    // Observe AppKit's update boundary rather than guessing a timer delay;
    // unchanged graphs require no writes and cannot disturb an open menu.
    if let Some(help) = submenu(&menu, "帮助")
        && different(application.helpMenu(), &help)
    {
        application.setHelpMenu(Some(&help));
    }
    if let Some(windows) = submenu(&menu, "窗口") {
        if let Some(front) = windows.itemWithTitle(&NSString::from_str("前置全部窗口")) {
            // SAFETY: AppKit's standard application window-ordering action.
            unsafe {
                front.setTarget(None);
                front.setAction(Some(sel!(arrangeInFront:)));
            }
        }
        if different(application.windowsMenu(), &windows) {
            application.setWindowsMenu(Some(&windows));
        }
    }
    if let Some(app) = submenu(&menu, "ChipDock")
        && let Some(services) = submenu(&app, "服务")
        && different(application.servicesMenu(), &services)
    {
        application.setServicesMenu(Some(&services));
    }
    validate_text_roles(&application, &menu);
    guard_modal_window_commands(&application, &menu);
}

fn validate_text_roles(application: &NSApplication, menu: &NSMenu) {
    let Some(edit) = submenu(menu, "编辑") else {
        return;
    };
    // SAFETY: Resolve the same standard action the nil-target menu sends,
    // including while AppKit tracks a menu over an auxiliary window.
    let responder = unsafe { application.targetForAction(sel!(copy:)) }
        .and_then(|target| target.downcast::<NSResponder>().ok());
    // Native text inputs keep AppKit's own editing/selection validation. The
    // generic NSH view instead always responds to Copy/Select All, even with
    // no selectable target; its standard Services API reports real selection.
    let nsh_view = responder
        .as_ref()
        .is_some_and(|responder| responder.class().name().to_bytes() == b"NshView");
    let selection = nsh_view
        && responder.as_ref().is_some_and(|responder| {
            // SAFETY: This immutable AppKit pasteboard-type constant has process lifetime.
            responder
                .validRequestorForSendType_returnType(Some(unsafe { NSPasteboardTypeString }), None)
                .is_some()
        });
    let mut changed = false;
    for (label, action, unavailable) in [
        ("拷贝", sel!(copy:), sel!(chipdockCopyUnavailable:)),
        (
            "全选",
            sel!(selectAll:),
            sel!(chipdockSelectAllUnavailable:),
        ),
    ] {
        if let Some(item) = edit.itemWithTitle(&NSString::from_str(label))
            && (item.action() == Some(action) || item.action() == Some(unavailable))
        {
            let wanted = if nsh_view && !selection {
                unavailable
            } else {
                action
            };
            if item.action() != Some(wanted) {
                // SAFETY: Standard action, or an intentionally unimplemented
                // selector that AppKit disables. Never changes custom Rust
                // collection handlers or an app-disabled menu item.
                unsafe {
                    item.setAction(Some(wanted));
                }
                changed = true;
            }
        }
    }
    if changed {
        edit.update();
    }
}

fn modal_targets(menu: &NSMenu, items: &mut Vec<Retained<NSMenuItem>>) {
    for item in menu.itemArray().iter() {
        let title = item.title().to_string();
        if item.action() == Some(sel!(toggleFullScreen:))
            || item.action() == Some(sel!(chipdockModalWindowCommand:))
            || title == "全屏幕平铺"
        {
            items.push(item.clone());
        }
        if let Some(submenu) = item.submenu() {
            modal_targets(&submenu, items);
        }
    }
}

/// A retained NSH sheet has no AppKit attachedSheet to disable the parent's
/// traffic light. Guard only ChipDock's main close button, retaining its value.
fn guard_modal_close_button(application: &NSApplication) {
    let modal = MAIN_MODAL.load(Ordering::Acquire) || application.modalWindow().is_some();
    if modal {
        MODAL_CLOSE.with(|saved| {
            let mut saved = saved.borrow_mut();
            if saved.is_none()
                && let Some(window) = application
                    .windows()
                    .iter()
                    .find(|window| window.title().to_string() == "ChipDock")
                && let Some(button) = window.standardWindowButton(NSWindowButton::CloseButton)
                && button.isEnabled()
            {
                // Do not take ownership of a button AppKit already disabled
                // for a native sheet: AppKit restores that button itself.
                *saved = Some(ModalClose {
                    enabled: button.isEnabled(),
                    button,
                });
            }
            if let Some(previous) = saved.as_ref() {
                previous.button.setEnabled(false);
            }
        });
    } else if let Some(previous) = MODAL_CLOSE.with(|saved| saved.borrow_mut().take()) {
        previous.button.setEnabled(previous.enabled);
    }
}

fn guard_modal_window_commands(application: &NSApplication, menu: &NSMenu) {
    let modal = application.modalWindow().is_some() || MAIN_MODAL.load(Ordering::Acquire);
    if modal {
        let mut targets = Vec::new();
        modal_targets(menu, &mut targets);
        MODAL_ITEMS.with(|guarded| {
            let mut guarded = guarded.borrow_mut();
            for item in &targets {
                if !guarded.iter().any(|old| std::ptr::eq(&*old.item, &**item)) {
                    guarded.push(ModalItem {
                        item: item.clone(),
                        action: item.action(),
                        enabled: item.isEnabled(),
                    });
                }
            }
        });
        for item in targets {
            // SAFETY: No responder implements this modal-disabled action.
            unsafe {
                item.setAction(Some(sel!(chipdockModalWindowCommand:)));
            }
            item.setEnabled(false);
        }
    } else {
        let previous = MODAL_ITEMS.with(|guarded| std::mem::take(&mut *guarded.borrow_mut()));
        for old in previous {
            // SAFETY: Restores the exact selector held by this same native
            // item before the guard, without changing any target or handler.
            unsafe {
                old.item.setAction(old.action);
            }
            old.item.setEnabled(old.enabled);
            // SAFETY: item.menu is its owning menu, if it still belongs to one.
            if let Some(parent) = unsafe { old.item.menu() } {
                parent.update();
            }
        }
    }
}
