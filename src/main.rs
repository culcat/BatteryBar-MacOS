mod battery;
mod updater;

use battery::{fetch_battery_info, BatteryInfo};
use objc2::rc::Retained;
use objc2::{declare_class, mutability, sel, ClassType, DeclaredClass};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSMenu, NSMenuItem, NSStatusBar,
    NSStatusItem, NSVariableStatusItemLength,
};
use objc2_foundation::{MainThreadBound, MainThreadMarker, NSObject, NSString, run_on_main};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use updater::{check_for_updates, open_release_url, UpdateInfo};

const UPDATE_INTERVAL_SECS: u64 = 30;

struct MenuRefs {
    status_item: Retained<NSStatusItem>,
    item_level: Retained<NSMenuItem>,
    item_time: Retained<NSMenuItem>,
    item_source: Retained<NSMenuItem>,
    item_update: Retained<NSMenuItem>,
}

struct AppState {
    menu_refs: MainThreadBound<MenuRefs>,
    latest_update: Mutex<Option<UpdateInfo>>,
}

static GLOBAL_STATE: Mutex<Option<Arc<AppState>>> = Mutex::new(None);

fn get_app_state() -> Option<Arc<AppState>> {
    GLOBAL_STATE.lock().unwrap().clone()
}

fn update_battery_display(refs: &MenuRefs, info: &BatteryInfo, mtm: MainThreadMarker) {
    // 1. Status bar button title
    if let Some(button) = unsafe { refs.status_item.button(mtm) } {
        let title = NSString::from_str(&info.menu_bar_text());
        unsafe { button.setTitle(&title) };
    }

    // 2. Details menu items
    let level_str = format!("Battery Level: {}% ({:?})", info.percentage, info.state);
    unsafe { refs.item_level.setTitle(&NSString::from_str(&level_str)) };

    let time_str = info.remaining_time.as_deref().unwrap_or("Calculating...");
    let remaining_str = format!("Time Remaining: {}", time_str);
    unsafe { refs.item_time.setTitle(&NSString::from_str(&remaining_str)) };

    let source_str = format!("Power Source: {}", info.power_source);
    unsafe { refs.item_source.setTitle(&NSString::from_str(&source_str)) };
}

fn trigger_battery_refresh() {
    std::thread::spawn(|| {
        if let Ok(info) = fetch_battery_info() {
            if let Some(state) = get_app_state() {
                run_on_main(move |mtm| {
                    let refs = state.menu_refs.get(mtm);
                    update_battery_display(refs, &info, mtm);
                });
            }
        }
    });
}

fn trigger_check_updates(manual: bool) {
    std::thread::spawn(move || {
        if manual {
            // Show checking in progress
            if let Some(state) = get_app_state() {
                let checking_text = "⏳ Checking for updates...";
                run_on_main(move |mtm| {
                    let refs = state.menu_refs.get(mtm);
                    unsafe { refs.item_update.setTitle(&NSString::from_str(checking_text)) };
                });
            }
        }

        match check_for_updates() {
            Ok(info) => {
                let is_newer = info.has_update;
                let release_url = info.release_url.clone();
                let latest_ver = info.latest_version.clone();
                let cur_ver = info.current_version.clone();

                if let Some(state) = get_app_state() {
                    *state.latest_update.lock().unwrap() = Some(info);

                    run_on_main(move |mtm| {
                        let refs = state.menu_refs.get(mtm);
                        if is_newer {
                            let text = format!("✨ Update Available (v{}) — Download", latest_ver);
                            unsafe {
                                refs.item_update.setTitle(&NSString::from_str(&text));
                                refs.item_update.setEnabled(true);
                            };
                            if manual {
                                open_release_url(&release_url);
                            }
                        } else {
                            let text = format!("✓ Version {} is up to date", cur_ver);
                            unsafe {
                                refs.item_update.setTitle(&NSString::from_str(&text));
                                refs.item_update.setEnabled(false);
                            };
                        }
                    });
                }
            }
            Err(err) => {
                eprintln!("Failed to check for updates: {}", err);
                if manual {
                    if let Some(state) = get_app_state() {
                        let text = "⚠️ Check failed (offline)";
                        run_on_main(move |mtm| {
                            let refs = state.menu_refs.get(mtm);
                            unsafe { refs.item_update.setTitle(&NSString::from_str(text)) };
                        });
                    }
                }
            }
        }
    });
}

declare_class!(
    struct MenuActionHandler;

    unsafe impl ClassType for MenuActionHandler {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "BatteryBarMenuActionHandler";
    }

    impl DeclaredClass for MenuActionHandler {
        type Ivars = ();
    }

    unsafe impl MenuActionHandler {
        #[method(refreshBattery:)]
        fn refresh_battery(&self, _sender: &NSMenuItem) {
            trigger_battery_refresh();
        }

        #[method(checkUpdates:)]
        fn check_updates(&self, _sender: &NSMenuItem) {
            if let Some(state) = get_app_state() {
                let update_opt = state.latest_update.lock().unwrap().clone();
                if let Some(info) = update_opt {
                    if info.has_update {
                        open_release_url(&info.release_url);
                        return;
                    }
                }
            }
            trigger_check_updates(true);
        }
    }
);

fn main() {
    let mtm = MainThreadMarker::new().expect("Must run on main thread");

    // Initialize NSApplication (Accessory mode: only menu bar, no dock)
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    // Initialize Action Handler
    let handler: Retained<MenuActionHandler> =
        unsafe { objc2::msg_send_id![MenuActionHandler::alloc(), init] };

    // Create system status bar item
    let status_bar = unsafe { NSStatusBar::systemStatusBar() };
    let status_item = unsafe { status_bar.statusItemWithLength(NSVariableStatusItemLength) };

    if let Some(button) = unsafe { status_item.button(mtm) } {
        let initial_title = NSString::from_str("🔋 --:--");
        unsafe { button.setTitle(&initial_title) };
    }

    // Build Dropdown Menu
    let menu = NSMenu::new(mtm);

    // 1. Info items
    let item_level = unsafe {
        menu.addItemWithTitle_action_keyEquivalent(
            &NSString::from_str("Battery Level: --"),
            None,
            &NSString::from_str(""),
        )
    };
    unsafe { item_level.setEnabled(false) };

    let item_time = unsafe {
        menu.addItemWithTitle_action_keyEquivalent(
            &NSString::from_str("Time Remaining: --"),
            None,
            &NSString::from_str(""),
        )
    };
    unsafe { item_time.setEnabled(false) };

    let item_source = unsafe {
        menu.addItemWithTitle_action_keyEquivalent(
            &NSString::from_str("Power Source: --"),
            None,
            &NSString::from_str(""),
        )
    };
    unsafe { item_source.setEnabled(false) };

    menu.addItem(&NSMenuItem::separatorItem(mtm));

    // 2. Manual Refresh item
    let item_refresh = unsafe {
        menu.addItemWithTitle_action_keyEquivalent(
            &NSString::from_str("🔄 Refresh Battery Status"),
            Some(sel!(refreshBattery:)),
            &NSString::from_str("r"),
        )
    };
    unsafe { item_refresh.setTarget(Some(&handler)) };

    // 3. Update info item
    let version_str = format!("⚡ Check for Updates... (v{})", env!("CARGO_PKG_VERSION"));
    let item_update = unsafe {
        menu.addItemWithTitle_action_keyEquivalent(
            &NSString::from_str(&version_str),
            Some(sel!(checkUpdates:)),
            &NSString::from_str("u"),
        )
    };
    unsafe { item_update.setTarget(Some(&handler)) };

    menu.addItem(&NSMenuItem::separatorItem(mtm));

    // 4. Update Interval hint
    let item_interval = unsafe {
        menu.addItemWithTitle_action_keyEquivalent(
            &NSString::from_str(&format!("Auto-updates every {}s", UPDATE_INTERVAL_SECS)),
            None,
            &NSString::from_str(""),
        )
    };
    unsafe { item_interval.setEnabled(false) };

    // 5. Quit item
    unsafe {
        menu.addItemWithTitle_action_keyEquivalent(
            &NSString::from_str("Quit"),
            Some(sel!(terminate:)),
            &NSString::from_str("q"),
        )
    };

    unsafe { status_item.setMenu(Some(&menu)) };

    let menu_refs = MenuRefs {
        status_item,
        item_level,
        item_time,
        item_source,
        item_update,
    };

    let app_state = Arc::new(AppState {
        menu_refs: MainThreadBound::new(menu_refs, mtm),
        latest_update: Mutex::new(None),
    });

    *GLOBAL_STATE.lock().unwrap() = Some(Arc::clone(&app_state));

    // Initial battery fetch
    if let Ok(initial_info) = fetch_battery_info() {
        update_battery_display(app_state.menu_refs.get(mtm), &initial_info, mtm);
    }

    // Background thread for periodic battery refresh
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(UPDATE_INTERVAL_SECS));

            if let Ok(info) = fetch_battery_info() {
                if let Some(state) = get_app_state() {
                    run_on_main(move |mtm| {
                        let refs = state.menu_refs.get(mtm);
                        update_battery_display(refs, &info, mtm);
                    });
                }
            }
        }
    });

    // Check for updates in background shortly after launch (after 4 seconds)
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(4));
        trigger_check_updates(false);
    });

    // Run native Cocoa event loop
    unsafe { app.run() };
}
