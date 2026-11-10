mod battery;
mod ui;
mod updater;

use battery::{fetch_battery_info, BatteryInfo, BatteryState, DisplayMode};
use objc2::rc::Retained;
use objc2::{declare_class, mutability, sel, ClassType, DeclaredClass};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSCellImagePosition, NSControlStateValueOff,
    NSControlStateValueOn, NSFont, NSFontWeightRegular, NSMenu, NSMenuItem, NSStatusBar,
    NSStatusItem, NSVariableStatusItemLength,
};
use objc2_foundation::{MainThreadBound, MainThreadMarker, NSObject, NSString, run_on_main};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use ui::{load_sf_symbol, HeaderCardView};
use updater::{check_for_updates, open_release_url, UpdateInfo};

const UPDATE_INTERVAL_SECS: u64 = 30;

struct MenuRefs {
    status_item: Retained<NSStatusItem>,
    header_card: HeaderCardView,
    item_health: Retained<NSMenuItem>,
    item_cycles: Retained<NSMenuItem>,
    item_power: Retained<NSMenuItem>,
    mode_items: [Retained<NSMenuItem>; 4],
    item_update: Retained<NSMenuItem>,
}

struct AppState {
    menu_refs: MainThreadBound<MenuRefs>,
    display_mode: Mutex<DisplayMode>,
    last_battery_info: Mutex<Option<BatteryInfo>>,
    latest_update: Mutex<Option<UpdateInfo>>,
}

static GLOBAL_STATE: Mutex<Option<Arc<AppState>>> = Mutex::new(None);

fn get_app_state() -> Option<Arc<AppState>> {
    GLOBAL_STATE.lock().unwrap().clone()
}

fn update_battery_display(
    refs: &MenuRefs,
    info: &BatteryInfo,
    mode: DisplayMode,
    mtm: MainThreadMarker,
) {
    // 1. Update Menu Bar Status Button (SF Symbol + Monospaced Digit Title)
    if let Some(button) = unsafe { refs.status_item.button(mtm) } {
        let title_str = info.menu_bar_title(mode);
        unsafe {
            if let Some(icon) = load_sf_symbol(info.sf_symbol_name(), Some(info.fallback_sf_symbol_name())) {
                button.setImage(Some(&icon));
                button.setImagePosition(NSCellImagePosition::NSImageLeading);
                button.setImageHugsTitle(true);
            }
            let font = NSFont::monospacedDigitSystemFontOfSize_weight(13.0, NSFontWeightRegular);
            button.setFont(Some(&font));
            button.setTitle(&NSString::from_str(&title_str));
        }
    }

    // 2. Update Control Center–style Header Card
    let is_charging = matches!(info.state, BatteryState::Charging | BatteryState::Charged);
    refs.header_card
        .progress_bar
        .update(info.percentage, is_charging);

    unsafe {
        refs.header_card
            .percentage_label
            .setStringValue(&NSString::from_str(&format!("{}%", info.percentage)));
        refs.header_card
            .status_label
            .setStringValue(&NSString::from_str(&info.primary_status_line()));
        refs.header_card
            .source_label
            .setStringValue(&NSString::from_str(&info.source_line()));
    }

    // 3. Update Hardware Telemetry Section
    let health_text = match (
        info.health_percent,
        info.nominal_capacity_mah,
        info.design_capacity_mah,
    ) {
        (Some(pct), Some(nom), Some(des)) => {
            format!("Battery Health: {}% ({} / {} mAh)", pct, nom, des)
        }
        (Some(pct), _, _) => format!("Battery Health: {}%", pct),
        _ => "Battery Health: Normal".to_string(),
    };
    unsafe { refs.item_health.setTitle(&NSString::from_str(&health_text)) };

    let cycles_text = match info.cycle_count {
        Some(c) => format!("Cycle Count: {} cycles", c),
        None => "Cycle Count: --".to_string(),
    };
    unsafe { refs.item_cycles.setTitle(&NSString::from_str(&cycles_text)) };

    let power_text = match (info.power_watts, info.voltage_volts) {
        (Some(w), Some(v)) if w > 0.05 => {
            let prefix = if info.state == BatteryState::Charging {
                "Charging Rate"
            } else {
                "Power Draw"
            };
            format!("{}: {:.1} W ({:.1} V)", prefix, w, v)
        }
        (_, Some(v)) => format!("Voltage: {:.2} V", v),
        _ => "Power Draw: --".to_string(),
    };
    unsafe { refs.item_power.setTitle(&NSString::from_str(&power_text)) };
}

fn update_mode_checkmarks(refs: &MenuRefs, active_mode: DisplayMode) {
    let active_idx = match active_mode {
        DisplayMode::Standard => 0,
        DisplayMode::Compact => 1,
        DisplayMode::TimeOnly => 2,
        DisplayMode::PercentageOnly => 3,
    };
    for (idx, item) in refs.mode_items.iter().enumerate() {
        let state = if idx == active_idx {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        };
        unsafe { item.setState(state) };
    }
}

fn trigger_battery_refresh() {
    std::thread::spawn(|| {
        if let Ok(info) = fetch_battery_info() {
            if let Some(state) = get_app_state() {
                let mode = *state.display_mode.lock().unwrap();
                *state.last_battery_info.lock().unwrap() = Some(info.clone());
                run_on_main(move |mtm| {
                    let refs = state.menu_refs.get(mtm);
                    update_battery_display(refs, &info, mode, mtm);
                });
            }
        }
    });
}

fn trigger_check_updates(manual: bool) {
    std::thread::spawn(move || {
        if manual {
            if let Some(state) = get_app_state() {
                let checking_text = "Checking for Updates…";
                run_on_main(move |mtm| {
                    let refs = state.menu_refs.get(mtm);
                    unsafe {
                        refs.item_update
                            .setTitle(&NSString::from_str(checking_text));
                    };
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
                            let text = format!("Update Available (v{}) — Download…", latest_ver);
                            unsafe {
                                refs.item_update.setTitle(&NSString::from_str(&text));
                                if let Some(icon) =
                                    load_sf_symbol("arrow.down.circle.fill", Some("arrow.down.circle"))
                                {
                                    refs.item_update.setImage(Some(&icon));
                                }
                                refs.item_update.setEnabled(true);
                            };
                            if manual {
                                open_release_url(&release_url);
                            }
                        } else {
                            let text = format!("Up to Date (v{})", cur_ver);
                            unsafe {
                                refs.item_update.setTitle(&NSString::from_str(&text));
                                if let Some(icon) =
                                    load_sf_symbol("checkmark.circle", Some("checkmark"))
                                {
                                    refs.item_update.setImage(Some(&icon));
                                }
                            };
                        }
                    });
                }
            }
            Err(err) => {
                eprintln!("Failed to check for updates: {}", err);
                if manual {
                    if let Some(state) = get_app_state() {
                        let text = "Update Check Failed (Offline)";
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

        #[method(selectDisplayMode:)]
        fn select_display_mode(&self, sender: &NSMenuItem) {
            let tag = unsafe { sender.tag() };
            let new_mode = match tag {
                1 => DisplayMode::Compact,
                2 => DisplayMode::TimeOnly,
                3 => DisplayMode::PercentageOnly,
                _ => DisplayMode::Standard,
            };

            if let Some(state) = get_app_state() {
                *state.display_mode.lock().unwrap() = new_mode;
                let cached_info = state.last_battery_info.lock().unwrap().clone();
                run_on_main(move |mtm| {
                    let refs = state.menu_refs.get(mtm);
                    update_mode_checkmarks(refs, new_mode);
                    if let Some(info) = cached_info {
                        update_battery_display(refs, &info, new_mode, mtm);
                    }
                });
            }
        }

        #[method(openBatterySettings:)]
        fn open_battery_settings(&self, _sender: &NSMenuItem) {
            let _ = Command::new("open")
                .arg("x-apple.systempreferences:com.apple.Battery-Settings.extension")
                .spawn();
        }
    }
);

fn set_item_sf_symbol(item: &NSMenuItem, primary: &str, fallback: Option<&str>) {
    if let Some(icon) = load_sf_symbol(primary, fallback) {
        unsafe { item.setImage(Some(&icon)) };
    }
}

fn main() {
    let mtm = MainThreadMarker::new().expect("Must run on main thread");

    // Initialize NSApplication in Accessory mode (menu bar only, no Dock icon)
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let handler: Retained<MenuActionHandler> =
        unsafe { objc2::msg_send_id![MenuActionHandler::alloc(), init] };

    // Create system status bar item
    let status_bar = unsafe { NSStatusBar::systemStatusBar() };
    let status_item = unsafe { status_bar.statusItemWithLength(NSVariableStatusItemLength) };

    if let Some(button) = unsafe { status_item.button(mtm) } {
        unsafe {
            if let Some(icon) = load_sf_symbol("battery.100percent", Some("battery.100")) {
                button.setImage(Some(&icon));
                button.setImagePosition(NSCellImagePosition::NSImageLeading);
                button.setImageHugsTitle(true);
            }
            let font = NSFont::monospacedDigitSystemFontOfSize_weight(13.0, NSFontWeightRegular);
            button.setFont(Some(&font));
            button.setTitle(&NSString::from_str("--%"));
        }
    }

    // Build Native Dropdown Menu
    let menu = NSMenu::new(mtm);

    // 1. Control Center–style Custom Header Card
    let header_card = HeaderCardView::new(mtm);
    let item_header = unsafe {
        menu.addItemWithTitle_action_keyEquivalent(
            &NSString::from_str(""),
            None,
            &NSString::from_str(""),
        )
    };
    unsafe { item_header.setView(Some(&header_card.container)) };

    menu.addItem(&NSMenuItem::separatorItem(mtm));

    // 2. Hardware Telemetry Section (with SF Symbols)
    let item_health = unsafe {
        menu.addItemWithTitle_action_keyEquivalent(
            &NSString::from_str("Battery Health: --"),
            None,
            &NSString::from_str(""),
        )
    };
    set_item_sf_symbol(&item_health, "bolt.heart", Some("heart"));
    unsafe { item_health.setEnabled(false) };

    let item_cycles = unsafe {
        menu.addItemWithTitle_action_keyEquivalent(
            &NSString::from_str("Cycle Count: --"),
            None,
            &NSString::from_str(""),
        )
    };
    set_item_sf_symbol(&item_cycles, "arrow.2.squarepath", Some("repeat"));
    unsafe { item_cycles.setEnabled(false) };

    let item_power = unsafe {
        menu.addItemWithTitle_action_keyEquivalent(
            &NSString::from_str("Power Draw: --"),
            None,
            &NSString::from_str(""),
        )
    };
    set_item_sf_symbol(&item_power, "bolt", None);
    unsafe { item_power.setEnabled(false) };

    menu.addItem(&NSMenuItem::separatorItem(mtm));

    // 3. Menu Bar Display Mode Submenu
    let item_display_menu = unsafe {
        menu.addItemWithTitle_action_keyEquivalent(
            &NSString::from_str("Menu Bar Display"),
            None,
            &NSString::from_str(""),
        )
    };
    set_item_sf_symbol(&item_display_menu, "slider.horizontal.3", None);

    let display_submenu = NSMenu::new(mtm);
    let mode_specs = [
        (0isize, "Time & Percentage (3h 15m · 76%)"),
        (1isize, "Compact (3:15 · 76%)"),
        (2isize, "Time Remaining Only (3h 15m)"),
        (3isize, "Percentage Only (76%)"),
    ];
    let mode_items = mode_specs.map(|(tag, label)| unsafe {
        let item = display_submenu.addItemWithTitle_action_keyEquivalent(
            &NSString::from_str(label),
            Some(sel!(selectDisplayMode:)),
            &NSString::from_str(""),
        );
        item.setTag(tag);
        item.setTarget(Some(&handler));
        if tag == 0 {
            item.setState(NSControlStateValueOn);
        }
        item
    });
    item_display_menu.setSubmenu(Some(&display_submenu));

    // 4. Action Items (Refresh, Updates, System Settings)
    let item_refresh = unsafe {
        menu.addItemWithTitle_action_keyEquivalent(
            &NSString::from_str("Refresh Now"),
            Some(sel!(refreshBattery:)),
            &NSString::from_str("r"),
        )
    };
    set_item_sf_symbol(&item_refresh, "arrow.clockwise", None);
    unsafe { item_refresh.setTarget(Some(&handler)) };

    let version_str = format!("Check for Updates… (v{})", env!("CARGO_PKG_VERSION"));
    let item_update = unsafe {
        menu.addItemWithTitle_action_keyEquivalent(
            &NSString::from_str(&version_str),
            Some(sel!(checkUpdates:)),
            &NSString::from_str("u"),
        )
    };
    set_item_sf_symbol(
        &item_update,
        "arrow.triangle.2.circlepath",
        Some("arrow.clockwise.circle"),
    );
    unsafe { item_update.setTarget(Some(&handler)) };

    let item_settings = unsafe {
        menu.addItemWithTitle_action_keyEquivalent(
            &NSString::from_str("Battery Settings…"),
            Some(sel!(openBatterySettings:)),
            &NSString::from_str(","),
        )
    };
    set_item_sf_symbol(&item_settings, "gearshape", Some("gear"));
    unsafe { item_settings.setTarget(Some(&handler)) };

    menu.addItem(&NSMenuItem::separatorItem(mtm));

    // 5. Quit Item
    unsafe {
        menu.addItemWithTitle_action_keyEquivalent(
            &NSString::from_str("Quit BatteryBar"),
            Some(sel!(terminate:)),
            &NSString::from_str("q"),
        )
    };

    unsafe { status_item.setMenu(Some(&menu)) };

    let menu_refs = MenuRefs {
        status_item,
        header_card,
        item_health,
        item_cycles,
        item_power,
        mode_items,
        item_update,
    };

    let app_state = Arc::new(AppState {
        menu_refs: MainThreadBound::new(menu_refs, mtm),
        display_mode: Mutex::new(DisplayMode::Standard),
        last_battery_info: Mutex::new(None),
        latest_update: Mutex::new(None),
    });

    *GLOBAL_STATE.lock().unwrap() = Some(Arc::clone(&app_state));

    // Initial battery fetch
    if let Ok(initial_info) = fetch_battery_info() {
        *app_state.last_battery_info.lock().unwrap() = Some(initial_info.clone());
        update_battery_display(
            app_state.menu_refs.get(mtm),
            &initial_info,
            DisplayMode::Standard,
            mtm,
        );
    }

    // Background thread for periodic battery refresh
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(UPDATE_INTERVAL_SECS));

            if let Ok(info) = fetch_battery_info() {
                if let Some(state) = get_app_state() {
                    let mode = *state.display_mode.lock().unwrap();
                    *state.last_battery_info.lock().unwrap() = Some(info.clone());
                    run_on_main(move |mtm| {
                        let refs = state.menu_refs.get(mtm);
                        update_battery_display(refs, &info, mode, mtm);
                    });
                }
            }
        }
    });

    // Check for updates in background shortly after launch
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(4));
        trigger_check_updates(false);
    });

    // Run native Cocoa event loop
    unsafe { app.run() };
}
