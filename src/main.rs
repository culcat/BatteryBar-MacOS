mod battery;

use battery::{fetch_battery_info, BatteryInfo};
use objc2::sel;
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSMenu, NSMenuItem, NSStatusBar,
    NSVariableStatusItemLength,
};
use objc2_foundation::{MainThreadBound, MainThreadMarker, NSString, run_on_main};
use std::sync::Arc;
use std::time::Duration;

const UPDATE_INTERVAL_SECS: u64 = 30;

fn update_ui(item: &objc2_app_kit::NSStatusItem, info: &BatteryInfo, mtm: MainThreadMarker) {
    // 1. Update status bar button title
    if let Some(button) = unsafe { item.button(mtm) } {
        let title = NSString::from_str(&info.menu_bar_text());
        unsafe { button.setTitle(&title) };
    }

    // 2. Update dropdown menu items
    if let Some(menu) = unsafe { item.menu(mtm) } {
        if let Some(level_item) = unsafe { menu.itemAtIndex(0) } {
            let text = NSString::from_str(&format!(
                "Battery Level: {}% ({:?})",
                info.percentage, info.state
            ));
            unsafe { level_item.setTitle(&text) };
        }

        if let Some(time_item) = unsafe { menu.itemAtIndex(1) } {
            let time_str = info.remaining_time.as_deref().unwrap_or("Calculating...");
            let text = NSString::from_str(&format!("Time Remaining: {}", time_str));
            unsafe { time_item.setTitle(&text) };
        }

        if let Some(source_item) = unsafe { menu.itemAtIndex(2) } {
            let text = NSString::from_str(&format!("Power Source: {}", info.power_source));
            unsafe { source_item.setTitle(&text) };
        }
    }
}

fn main() {
    let mtm = MainThreadMarker::new().expect("Must run on main thread");

    // Initialize NSApplication and hide dock icon (menu bar app only)
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    // Create system status bar item
    let status_bar = unsafe { NSStatusBar::systemStatusBar() };
    let status_item = unsafe { status_bar.statusItemWithLength(NSVariableStatusItemLength) };

    // Set initial placeholder title
    if let Some(button) = unsafe { status_item.button(mtm) } {
        let initial_title = NSString::from_str("🔋 --:--");
        unsafe { button.setTitle(&initial_title) };
    }

    // Build dropdown menu
    let menu = NSMenu::new(mtm);

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

    // Separator
    let separator = NSMenuItem::separatorItem(mtm);
    menu.addItem(&separator);

    // Refresh interval notice
    let item_interval = unsafe {
        menu.addItemWithTitle_action_keyEquivalent(
            &NSString::from_str(&format!("Updates every {}s", UPDATE_INTERVAL_SECS)),
            None,
            &NSString::from_str(""),
        )
    };
    unsafe { item_interval.setEnabled(false) };

    // Quit item
    unsafe {
        menu.addItemWithTitle_action_keyEquivalent(
            &NSString::from_str("Quit"),
            Some(sel!(terminate:)),
            &NSString::from_str("q"),
        )
    };

    unsafe { status_item.setMenu(Some(&menu)) };

    // Initial update right away
    if let Ok(initial_info) = fetch_battery_info() {
        update_ui(&status_item, &initial_info, mtm);
    }

    // Wrap status_item in MainThreadBound so it can be safely referenced across threads
    let item_bound = Arc::new(MainThreadBound::new(status_item, mtm));

    // Background thread for periodic updates
    let item_clone = Arc::clone(&item_bound);
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(UPDATE_INTERVAL_SECS));

            match fetch_battery_info() {
                Ok(info) => {
                    let item_ref = Arc::clone(&item_clone);
                    run_on_main(move |mtm| {
                        let item = item_ref.get(mtm);
                        update_ui(item, &info, mtm);
                    });
                }
                Err(err) => {
                    eprintln!("Error fetching battery info: {}", err);
                }
            }
        }
    });

    // Run Cocoa event loop
    unsafe { app.run() };
}
