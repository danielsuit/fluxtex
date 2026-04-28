#[derive(Clone, Copy, Debug)]
pub enum AppMenuCommand {
    NewStarterDocument,
    OpenFile,
    SaveFile,
    SaveFileAs,
    ReloadFile,
    Compile,
    StopCompile,
    ToggleProjectSidebar,
    TogglePreview,
    ToggleProblems,
    NextTheme,
    PreviousTheme,
    PreviewZoomIn,
    PreviewZoomOut,
    PreviewResetZoom,
    PreviewToggleFitPage,
    ToggleFullscreen,
    ShowKeyboardShortcuts,
}

#[cfg(not(target_os = "macos"))]
pub fn install_app_menu(_sender: crossbeam_channel::Sender<AppMenuCommand>) {}

#[cfg(target_os = "macos")]
mod macos {
    use super::AppMenuCommand;
    use crossbeam_channel::Sender;
    use objc2::msg_send_id;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject, NSObject, Sel};
    use objc2::{class, sel};
    use objc2_app_kit::{
        NSApp, NSApplicationActivationPolicy, NSApplicationDidFinishLaunchingNotification,
        NSEventModifierFlags, NSMenu, NSMenuItem,
    };
    use objc2_foundation::{MainThreadMarker, NSInteger, NSNotificationCenter, NSString};
    use std::sync::{Mutex, Once, OnceLock};

    const APP_NAME: &str = "FluXTeX";

    // Tags for menu items routed through our action handler.
    const CMD_NEW_STARTER: NSInteger = 1;
    const CMD_OPEN_FILE: NSInteger = 2;
    const CMD_SAVE_FILE: NSInteger = 3;
    const CMD_SAVE_FILE_AS: NSInteger = 4;
    const CMD_RELOAD_FILE: NSInteger = 5;
    const CMD_COMPILE: NSInteger = 6;
    const CMD_STOP_COMPILE: NSInteger = 7;
    const CMD_TOGGLE_PROJECT_SIDEBAR: NSInteger = 8;
    const CMD_TOGGLE_PREVIEW: NSInteger = 9;
    const CMD_TOGGLE_PROBLEMS: NSInteger = 10;
    const CMD_NEXT_THEME: NSInteger = 11;
    const CMD_PREV_THEME: NSInteger = 12;
    const CMD_PREVIEW_ZOOM_IN: NSInteger = 13;
    const CMD_PREVIEW_ZOOM_OUT: NSInteger = 14;
    const CMD_PREVIEW_RESET_ZOOM: NSInteger = 15;
    const CMD_PREVIEW_TOGGLE_FIT: NSInteger = 16;
    const CMD_TOGGLE_FULLSCREEN: NSInteger = 17;
    const CMD_SHOW_KEYBOARD_SHORTCUTS: NSInteger = 18;
    const CMD_OPEN_DOCS: NSInteger = 19;
    const CMD_OPEN_REPO: NSInteger = 20;
    const CMD_REPORT_ISSUE: NSInteger = 21;

    static MENU_INSTALLED: Once = Once::new();
    static MENU_SENDER: OnceLock<Mutex<Option<Sender<AppMenuCommand>>>> = OnceLock::new();
    static MENU_TARGET_CLASS: OnceLock<&'static AnyClass> = OnceLock::new();
    static MENU_TARGET_PTR: OnceLock<usize> = OnceLock::new();

    pub fn install_app_menu(sender: Sender<AppMenuCommand>) {
        let lock = MENU_SENDER.get_or_init(|| Mutex::new(None));
        *lock.lock().unwrap() = Some(sender);

        MENU_INSTALLED.call_once(|| {
            let mtm = match MainThreadMarker::new() {
                Some(m) => m,
                None => {
                    tracing::warn!("install_app_menu: not on main thread; menu skipped");
                    return;
                }
            };

            unsafe {
                let app = NSApp(mtm);
                // Force "Regular" so the system shows the menu bar for non-bundled binaries.
                app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

                // Register an observer so we re-install our menu after AppKit/winit
                // finish launching. This is what wins against winit's default menu.
                let target = menu_target();
                let center = NSNotificationCenter::defaultCenter();
                center.addObserver_selector_name_object(
                    target,
                    sel!(handleFluxTexAppDidFinishLaunching:),
                    Some(NSApplicationDidFinishLaunchingNotification),
                    None,
                );

                // Install immediately too, to cover the case where launch already finished.
                install_menubar_now(mtm);
            }
        });
    }

    fn install_menubar_now(mtm: MainThreadMarker) {
        unsafe {
            let app = NSApp(mtm);
            let menubar = build_menubar(mtm);
            app.setMainMenu(Some(&menubar));
        }
    }

    unsafe fn build_menubar(mtm: MainThreadMarker) -> Retained<NSMenu> {
        let cmd = NSEventModifierFlags::NSEventModifierFlagCommand;
        let cmd_shift = NSEventModifierFlags::NSEventModifierFlagCommand
            | NSEventModifierFlags::NSEventModifierFlagShift;
        let cmd_opt = NSEventModifierFlags::NSEventModifierFlagCommand
            | NSEventModifierFlags::NSEventModifierFlagOption;
        let cmd_ctrl = NSEventModifierFlags::NSEventModifierFlagCommand
            | NSEventModifierFlags::NSEventModifierFlagControl;

        let menubar = NSMenu::new(mtm);

        // ── App menu ───────────────────────────────────────────────────
        menubar.addItem(&submenu(
            mtm,
            APP_NAME,
            &[
                MenuEntry::Item(standard(
                    mtm,
                    &format!("About {APP_NAME}"),
                    Some(sel!(orderFrontStandardAboutPanel:)),
                    "",
                    None,
                )),
                MenuEntry::Separator,
                MenuEntry::Item(standard(mtm, "Services", None, "", None)),
                MenuEntry::Separator,
                MenuEntry::Item(standard(
                    mtm,
                    &format!("Hide {APP_NAME}"),
                    Some(sel!(hide:)),
                    "h",
                    Some(cmd),
                )),
                MenuEntry::Item(standard(
                    mtm,
                    "Hide Others",
                    Some(sel!(hideOtherApplications:)),
                    "h",
                    Some(cmd_opt),
                )),
                MenuEntry::Item(standard(
                    mtm,
                    "Show All",
                    Some(sel!(unhideAllApplications:)),
                    "",
                    None,
                )),
                MenuEntry::Separator,
                MenuEntry::Item(standard(
                    mtm,
                    &format!("Quit {APP_NAME}"),
                    Some(sel!(terminate:)),
                    "q",
                    Some(cmd),
                )),
            ],
        ));

        // ── File ───────────────────────────────────────────────────────
        menubar.addItem(&submenu(
            mtm,
            "File",
            &[
                MenuEntry::Item(command(
                    mtm,
                    "New From Starter Template",
                    CMD_NEW_STARTER,
                    "n",
                    Some(cmd),
                )),
                MenuEntry::Separator,
                MenuEntry::Item(command(mtm, "Open…", CMD_OPEN_FILE, "o", Some(cmd))),
                MenuEntry::Item(command(
                    mtm,
                    "Reload From Disk",
                    CMD_RELOAD_FILE,
                    "r",
                    Some(cmd_shift),
                )),
                MenuEntry::Separator,
                MenuEntry::Item(command(mtm, "Save", CMD_SAVE_FILE, "s", Some(cmd))),
                MenuEntry::Item(command(
                    mtm,
                    "Save As…",
                    CMD_SAVE_FILE_AS,
                    "s",
                    Some(cmd_shift),
                )),
                MenuEntry::Separator,
                MenuEntry::Item(command(mtm, "Compile", CMD_COMPILE, "b", Some(cmd))),
                MenuEntry::Item(command(
                    mtm,
                    "Stop Compile",
                    CMD_STOP_COMPILE,
                    ".",
                    Some(cmd),
                )),
                MenuEntry::Separator,
                MenuEntry::Item(standard(
                    mtm,
                    "Page Setup…",
                    Some(sel!(runPageLayout:)),
                    "p",
                    Some(cmd_shift),
                )),
                MenuEntry::Item(standard(mtm, "Print…", Some(sel!(print:)), "p", Some(cmd))),
                MenuEntry::Separator,
                MenuEntry::Item(standard(
                    mtm,
                    "Close Window",
                    Some(sel!(performClose:)),
                    "w",
                    Some(cmd),
                )),
            ],
        ));

        // ── Edit ───────────────────────────────────────────────────────
        menubar.addItem(&submenu(
            mtm,
            "Edit",
            &[
                MenuEntry::Item(standard(mtm, "Undo", Some(sel!(undo:)), "z", Some(cmd))),
                MenuEntry::Item(standard(
                    mtm,
                    "Redo",
                    Some(sel!(redo:)),
                    "z",
                    Some(cmd_shift),
                )),
                MenuEntry::Separator,
                MenuEntry::Item(standard(mtm, "Cut", Some(sel!(cut:)), "x", Some(cmd))),
                MenuEntry::Item(standard(mtm, "Copy", Some(sel!(copy:)), "c", Some(cmd))),
                MenuEntry::Item(standard(mtm, "Paste", Some(sel!(paste:)), "v", Some(cmd))),
                MenuEntry::Item(standard(
                    mtm,
                    "Paste and Match Style",
                    Some(sel!(pasteAsPlainText:)),
                    "v",
                    Some(cmd_opt | NSEventModifierFlags::NSEventModifierFlagShift),
                )),
                MenuEntry::Item(standard(mtm, "Delete", Some(sel!(delete:)), "", None)),
                MenuEntry::Item(standard(
                    mtm,
                    "Select All",
                    Some(sel!(selectAll:)),
                    "a",
                    Some(cmd),
                )),
                MenuEntry::Separator,
                MenuEntry::SubItem(submenu(
                    mtm,
                    "Find",
                    &[
                        MenuEntry::Item(standard_with_tag(
                            mtm,
                            "Find…",
                            Some(sel!(performFindPanelAction:)),
                            "f",
                            Some(cmd),
                            1,
                        )),
                        MenuEntry::Item(standard_with_tag(
                            mtm,
                            "Find Next",
                            Some(sel!(performFindPanelAction:)),
                            "g",
                            Some(cmd),
                            2,
                        )),
                        MenuEntry::Item(standard_with_tag(
                            mtm,
                            "Find Previous",
                            Some(sel!(performFindPanelAction:)),
                            "g",
                            Some(cmd_shift),
                            3,
                        )),
                        MenuEntry::Item(standard_with_tag(
                            mtm,
                            "Use Selection for Find",
                            Some(sel!(performFindPanelAction:)),
                            "e",
                            Some(cmd),
                            7,
                        )),
                        MenuEntry::Item(standard_with_tag(
                            mtm,
                            "Jump to Selection",
                            Some(sel!(centerSelectionInVisibleArea:)),
                            "j",
                            Some(cmd),
                            0,
                        )),
                    ],
                )),
                MenuEntry::SubItem(submenu(
                    mtm,
                    "Spelling and Grammar",
                    &[
                        MenuEntry::Item(standard(
                            mtm,
                            "Show Spelling and Grammar",
                            Some(sel!(showGuessPanel:)),
                            ":",
                            Some(cmd),
                        )),
                        MenuEntry::Item(standard(
                            mtm,
                            "Check Document Now",
                            Some(sel!(checkSpelling:)),
                            ";",
                            Some(cmd),
                        )),
                    ],
                )),
            ],
        ));

        // ── View ───────────────────────────────────────────────────────
        menubar.addItem(&submenu(
            mtm,
            "View",
            &[
                MenuEntry::Item(command(
                    mtm,
                    "Toggle Project Sidebar",
                    CMD_TOGGLE_PROJECT_SIDEBAR,
                    "1",
                    Some(cmd),
                )),
                MenuEntry::Item(command(
                    mtm,
                    "Toggle Preview",
                    CMD_TOGGLE_PREVIEW,
                    "2",
                    Some(cmd),
                )),
                MenuEntry::Item(command(
                    mtm,
                    "Toggle Problems Panel",
                    CMD_TOGGLE_PROBLEMS,
                    "j",
                    Some(cmd_shift),
                )),
                MenuEntry::Separator,
                MenuEntry::Item(command(
                    mtm,
                    "Zoom Preview In",
                    CMD_PREVIEW_ZOOM_IN,
                    "=",
                    Some(cmd),
                )),
                MenuEntry::Item(command(
                    mtm,
                    "Zoom Preview Out",
                    CMD_PREVIEW_ZOOM_OUT,
                    "-",
                    Some(cmd),
                )),
                MenuEntry::Item(command(
                    mtm,
                    "Reset Preview Zoom",
                    CMD_PREVIEW_RESET_ZOOM,
                    "0",
                    Some(cmd),
                )),
                MenuEntry::Item(command(
                    mtm,
                    "Toggle Fit Page",
                    CMD_PREVIEW_TOGGLE_FIT,
                    "9",
                    Some(cmd),
                )),
                MenuEntry::Separator,
                MenuEntry::Item(command(
                    mtm,
                    "Next Theme",
                    CMD_NEXT_THEME,
                    "t",
                    Some(cmd_shift),
                )),
                MenuEntry::Item(command(
                    mtm,
                    "Previous Theme",
                    CMD_PREV_THEME,
                    "t",
                    Some(cmd_opt),
                )),
                MenuEntry::Separator,
                MenuEntry::Item(command(
                    mtm,
                    "Toggle Full Screen",
                    CMD_TOGGLE_FULLSCREEN,
                    "f",
                    Some(cmd_ctrl),
                )),
            ],
        ));

        // ── Build ──────────────────────────────────────────────────────
        menubar.addItem(&submenu(
            mtm,
            "Build",
            &[
                MenuEntry::Item(command(mtm, "Compile", CMD_COMPILE, "b", Some(cmd))),
                MenuEntry::Item(command(
                    mtm,
                    "Stop Compile",
                    CMD_STOP_COMPILE,
                    ".",
                    Some(cmd),
                )),
            ],
        ));

        // ── Window ─────────────────────────────────────────────────────
        let window_menu_item = submenu(
            mtm,
            "Window",
            &[
                MenuEntry::Item(standard(
                    mtm,
                    "Minimize",
                    Some(sel!(performMiniaturize:)),
                    "m",
                    Some(cmd),
                )),
                MenuEntry::Item(standard(mtm, "Zoom", Some(sel!(performZoom:)), "", None)),
                MenuEntry::Separator,
                MenuEntry::Item(standard(
                    mtm,
                    "Bring All to Front",
                    Some(sel!(arrangeInFront:)),
                    "",
                    None,
                )),
            ],
        );
        let app = NSApp(mtm);
        if let Some(menu) = window_menu_item.submenu() {
            app.setWindowsMenu(Some(&menu));
        }
        menubar.addItem(&window_menu_item);

        // ── Help ───────────────────────────────────────────────────────
        menubar.addItem(&submenu(
            mtm,
            "Help",
            &[
                MenuEntry::Item(command(
                    mtm,
                    "Keyboard Shortcuts",
                    CMD_SHOW_KEYBOARD_SHORTCUTS,
                    "/",
                    Some(cmd),
                )),
                MenuEntry::Separator,
                MenuEntry::Item(command(
                    mtm,
                    "FluXTeX Documentation",
                    CMD_OPEN_DOCS,
                    "",
                    None,
                )),
                MenuEntry::Item(command(mtm, "GitHub Repository", CMD_OPEN_REPO, "", None)),
                MenuEntry::Item(command(mtm, "Report an Issue", CMD_REPORT_ISSUE, "", None)),
            ],
        ));

        menubar
    }

    fn menu_target() -> &'static AnyObject {
        let ptr = *MENU_TARGET_PTR.get_or_init(|| {
            let cls = MENU_TARGET_CLASS.get_or_init(|| {
                let mut builder =
                    objc2::runtime::ClassBuilder::new("FluxTexMenuTarget", class!(NSObject))
                        .expect("failed to create macOS menu target class");
                unsafe {
                    builder.add_method(
                        sel!(handleFluxTexMenuItem:),
                        handle_menu_item as extern "C" fn(_, _, _),
                    );
                    builder.add_method(
                        sel!(handleFluxTexAppDidFinishLaunching:),
                        handle_did_finish_launching as extern "C" fn(_, _, _),
                    );
                }
                builder.register()
            });
            let target: Retained<AnyObject> = unsafe { msg_send_id![*cls, new] };
            Retained::into_raw(target) as usize
        });

        unsafe { &*(ptr as *const AnyObject) }
    }

    extern "C" fn handle_did_finish_launching(_this: &NSObject, _cmd: Sel, _note: &NSObject) {
        if let Some(mtm) = MainThreadMarker::new() {
            install_menubar_now(mtm);
        }
    }

    extern "C" fn handle_menu_item(_this: &NSObject, _cmd: Sel, item: &NSMenuItem) {
        let tag = unsafe { item.tag() };
        let command = match tag {
            CMD_NEW_STARTER => Some(AppMenuCommand::NewStarterDocument),
            CMD_OPEN_FILE => Some(AppMenuCommand::OpenFile),
            CMD_SAVE_FILE => Some(AppMenuCommand::SaveFile),
            CMD_SAVE_FILE_AS => Some(AppMenuCommand::SaveFileAs),
            CMD_RELOAD_FILE => Some(AppMenuCommand::ReloadFile),
            CMD_COMPILE => Some(AppMenuCommand::Compile),
            CMD_STOP_COMPILE => Some(AppMenuCommand::StopCompile),
            CMD_TOGGLE_PROJECT_SIDEBAR => Some(AppMenuCommand::ToggleProjectSidebar),
            CMD_TOGGLE_PREVIEW => Some(AppMenuCommand::TogglePreview),
            CMD_TOGGLE_PROBLEMS => Some(AppMenuCommand::ToggleProblems),
            CMD_NEXT_THEME => Some(AppMenuCommand::NextTheme),
            CMD_PREV_THEME => Some(AppMenuCommand::PreviousTheme),
            CMD_PREVIEW_ZOOM_IN => Some(AppMenuCommand::PreviewZoomIn),
            CMD_PREVIEW_ZOOM_OUT => Some(AppMenuCommand::PreviewZoomOut),
            CMD_PREVIEW_RESET_ZOOM => Some(AppMenuCommand::PreviewResetZoom),
            CMD_PREVIEW_TOGGLE_FIT => Some(AppMenuCommand::PreviewToggleFitPage),
            CMD_TOGGLE_FULLSCREEN => Some(AppMenuCommand::ToggleFullscreen),
            CMD_SHOW_KEYBOARD_SHORTCUTS => Some(AppMenuCommand::ShowKeyboardShortcuts),
            CMD_OPEN_DOCS => {
                open_url("https://github.com/danielsuit/fluxtex#readme");
                None
            }
            CMD_OPEN_REPO => {
                open_url("https://github.com/danielsuit/fluxtex");
                None
            }
            CMD_REPORT_ISSUE => {
                open_url("https://github.com/danielsuit/fluxtex/issues/new");
                None
            }
            _ => None,
        };

        if let Some(cmd) = command {
            if let Some(sender) = MENU_SENDER
                .get()
                .and_then(|lock| lock.lock().unwrap().clone())
            {
                let _ = sender.send(cmd);
            }
        }
    }

    fn open_url(url: &str) {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }

    unsafe fn standard(
        mtm: MainThreadMarker,
        title: &str,
        action: Option<Sel>,
        key: &str,
        modifiers: Option<NSEventModifierFlags>,
    ) -> Retained<NSMenuItem> {
        let item = NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc::<NSMenuItem>(),
            &NSString::from_str(title),
            action,
            &NSString::from_str(key),
        );
        if let Some(modifiers) = modifiers {
            item.setKeyEquivalentModifierMask(modifiers);
        }
        item
    }

    unsafe fn standard_with_tag(
        mtm: MainThreadMarker,
        title: &str,
        action: Option<Sel>,
        key: &str,
        modifiers: Option<NSEventModifierFlags>,
        tag: NSInteger,
    ) -> Retained<NSMenuItem> {
        let item = standard(mtm, title, action, key, modifiers);
        item.setTag(tag);
        item
    }

    unsafe fn command(
        mtm: MainThreadMarker,
        title: &str,
        tag: NSInteger,
        key: &str,
        modifiers: Option<NSEventModifierFlags>,
    ) -> Retained<NSMenuItem> {
        let item = standard(
            mtm,
            title,
            Some(sel!(handleFluxTexMenuItem:)),
            key,
            modifiers,
        );
        item.setTarget(Some(menu_target()));
        item.setTag(tag);
        item
    }

    unsafe fn submenu(
        mtm: MainThreadMarker,
        title: &str,
        entries: &[MenuEntry],
    ) -> Retained<NSMenuItem> {
        let menu = NSMenu::new(mtm);
        menu.setTitle(&NSString::from_str(title));
        for entry in entries {
            match entry {
                MenuEntry::Item(item) => menu.addItem(item),
                MenuEntry::SubItem(item) => menu.addItem(item),
                MenuEntry::Separator => menu.addItem(&NSMenuItem::separatorItem(mtm)),
            }
        }
        let host = NSMenuItem::new(mtm);
        host.setTitle(&NSString::from_str(title));
        host.setSubmenu(Some(&menu));
        host
    }

    enum MenuEntry {
        Item(Retained<NSMenuItem>),
        SubItem(Retained<NSMenuItem>),
        Separator,
    }
}

#[cfg(target_os = "macos")]
pub use macos::install_app_menu;
