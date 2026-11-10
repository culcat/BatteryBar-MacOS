mod battery;
mod i18n;
mod ui;
mod updater;

use battery::{fetch_battery_info, BatteryInfo, BatteryState, DisplayMode};
use i18n::Language;
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
    item_display_menu: Retained<NSMenuItem>,
    mode_items: [Retained<NSMenuItem>; 4],
    item_lang_menu: Retained<NSMenuItem>,
    lang_items: [Retained<NSMenuItem>; 5],
    item_refresh: Retained<NSMenuItem>,
    item_update: Retained<NSMenuItem>,
    item_settings: Retained<NSMenuItem>,
    item_quit: Retained<NSMenuItem>,
}

struct AppState {
    menu_refs: MainThreadBound<MenuRefs>,
    display_mode: Mutex<DisplayMode>,
    language: Mutex<Language>,
    last_battery_info: Mutex<Option<BatteryInfo>>,
    latest_update: Mutex<Option<UpdateInfo>>,
}

static GLOBAL_STATE: Mutex<Option<Arc<AppState>>> = Mutex::new(None);

fn get_app_state() -> Option<Arc<AppState>> {
    GLOBAL_STATE.lock().unwrap().clone()
}

fn update_static_menu_labels(
    refs: &MenuRefs,
    lang: Language,
    latest_update: Option<&UpdateInfo>,
) {
    unsafe {
        refs.header_card
            .title_label
            .setStringValue(&NSString::from_str(lang.battery_title()));

        refs.item_display_menu
            .setTitle(&NSString::from_str(lang.menu_bar_display()));

        let mode_labels = lang.display_mode_labels();
        for (item, label) in refs.mode_items.iter().zip(mode_labels.iter()) {
            item.setTitle(&NSString::from_str(label));
        }

        refs.item_lang_menu
            .setTitle(&NSString::from_str(lang.language_menu_title()));

        refs.item_refresh
            .setTitle(&NSString::from_str(lang.refresh_now()));

        let update_text = match latest_update {
            Some(u) if u.has_update => lang.update_available(&u.latest_version),
            Some(u) => lang.up_to_date(&u.current_version),
            None => lang.check_for_updates(env!("CARGO_PKG_VERSION")),
        };
        refs.item_update
            .setTitle(&NSString::from_str(&update_text));

        refs.item_settings
            .setTitle(&NSString::from_str(lang.battery_settings()));

        refs.item_quit
            .setTitle(&NSString::from_str(lang.quit_app()));
    }
}

fn update_battery_display(
    refs: &MenuRefs,
    info: &BatteryInfo,
    mode: DisplayMode,
    lang: Language,
    mtm: MainThreadMarker,
) {
    // 1. Update Menu Bar Status Button (SF Symbol + Monospaced Digit Title)
    if let Some(button) = unsafe { refs.status_item.button(mtm) } {
        let title_str = info.menu_bar_title_localized(mode, lang);
        unsafe {
            if let Some(icon) =
                load_sf_symbol(info.sf_symbol_name(), Some(info.fallback_sf_symbol_name()))
            {
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
            .title_label
            .setStringValue(&NSString::from_str(lang.battery_title()));
        refs.header_card
            .percentage_label
            .setStringValue(&NSString::from_str(&format!("{}%", info.percentage)));
        refs.header_card.status_label.setStringValue(
            &NSString::from_str(&info.primary_status_line_localized(lang)),
        );
        refs.header_card
            .source_label
            .setStringValue(&NSString::from_str(&info.source_line_localized(lang)));
    }

    // 3. Update Hardware Telemetry Section
    let health_text = lang.battery_health(
        info.health_percent,
        info.nominal_capacity_mah,
        info.design_capacity_mah,
    );
    unsafe { refs.item_health.setTitle(&NSString::from_str(&health_text)) };

    let cycles_text = lang.cycle_count(info.cycle_count);
    unsafe { refs.item_cycles.setTitle(&NSString::from_str(&cycles_text)) };

    let power_text = lang.power_metrics(
        info.power_watts,
        info.voltage_volts,
        info.state == BatteryState::Charging,
    );
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

fn update_language_checkmarks(refs: &MenuRefs, active_lang: Language) {
    let active_idx = match active_lang {
        Language::En => 0,
        Language::Ru => 1,
        Language::Fr => 2,
        Language::Es => 3,
        Language::Zh => 4,
    };
    for (idx, item) in refs.lang_items.iter().enumerate() {
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
                let lang = *state.language.lock().unwrap();
                *state.last_battery_info.lock().unwrap() = Some(info.clone());
                run_on_main(move |mtm| {
                    let refs = state.menu_refs.get(mtm);
                    update_battery_display(refs, &info, mode, lang, mtm);
                });
            }
        }
    });
}

fn trigger_check_updates(manual: bool) {
    std::thread::spawn(move || {
        if manual {
            if let Some(state) = get_app_state() {
                let lang = *state.language.lock().unwrap();
                let checking_text = lang.checking_updates();
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
                    let lang = *state.language.lock().unwrap();
                    *state.latest_update.lock().unwrap() = Some(info);

                    run_on_main(move |mtm| {
                        let refs = state.menu_refs.get(mtm);
                        if is_newer {
                            let text = lang.update_available(&latest_ver);
                            unsafe {
                                refs.item_update.setTitle(&NSString::from_str(&text));
                                if let Some(icon) = load_sf_symbol(
                                    "arrow.down.circle.fill",
                                    Some("arrow.down.circle"),
                                ) {
                                    refs.item_update.setImage(Some(&icon));
                                }
                                refs.item_update.setEnabled(true);
                            };
                            if manual {
                                open_release_url(&release_url);
                            }
                        } else {
                            let text = lang.up_to_date(&cur_ver);
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
                        let lang = *state.language.lock().unwrap();
                        let text = lang.update_failed_offline();
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
                let lang = *state.language.lock().unwrap();
                let cached_info = state.last_battery_info.lock().unwrap().clone();
                run_on_main(move |mtm| {
                    let refs = state.menu_refs.get(mtm);
                    update_mode_checkmarks(refs, new_mode);
                    if let Some(info) = cached_info {
                        update_battery_display(refs, &info, new_mode, lang, mtm);
                    }
                });
            }
        }

        #[method(selectLanguage:)]
        fn select_language(&self, sender: &NSMenuItem) {
            let tag = unsafe { sender.tag() };
            let new_lang = match tag {
                1 => Language::Ru,
                2 => Language::Fr,
                3 => Language::Es,
                4 => Language::Zh,
                _ => Language::En,
            };

            if let Some(state) = get_app_state() {
                *state.language.lock().unwrap() = new_lang;
                let mode = *state.display_mode.lock().unwrap();
                let cached_info = state.last_battery_info.lock().unwrap().clone();
                let cached_update = state.latest_update.lock().unwrap().clone();
                run_on_main(move |mtm| {
                    let refs = state.menu_refs.get(mtm);
                    update_language_checkmarks(refs, new_lang);
                    update_static_menu_labels(refs, new_lang, cached_update.as_ref());
                    if let Some(info) = cached_info {
                        update_battery_display(refs, &info, mode, new_lang, mtm);
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
    let initial_lang = Language::detect_system();

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
            &NSString::from_str(&initial_lang.battery_health(None, None, None)),
            None,
            &NSString::from_str(""),
        )
    };
    set_item_sf_symbol(&item_health, "bolt.heart", Some("heart"));
    unsafe { item_health.setEnabled(false) };

    let item_cycles = unsafe {
        menu.addItemWithTitle_action_keyEquivalent(
            &NSString::from_str(&initial_lang.cycle_count(None)),
            None,
            &NSString::from_str(""),
        )
    };
    set_item_sf_symbol(&item_cycles, "arrow.2.squarepath", Some("repeat"));
    unsafe { item_cycles.setEnabled(false) };

    let item_power = unsafe {
        menu.addItemWithTitle_action_keyEquivalent(
            &NSString::from_str(&initial_lang.power_metrics(None, None, false)),
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
            &NSString::from_str(initial_lang.menu_bar_display()),
            None,
            &NSString::from_str(""),
        )
    };
    set_item_sf_symbol(&item_display_menu, "slider.horizontal.3", None);

    let display_submenu = NSMenu::new(mtm);
    let initial_mode_labels = initial_lang.display_mode_labels();
    let mode_specs = [
        (0isize, initial_mode_labels[0]),
        (1isize, initial_mode_labels[1]),
        (2isize, initial_mode_labels[2]),
        (3isize, initial_mode_labels[3]),
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

    // 3b. Language Submenu
    let item_lang_menu = unsafe {
        menu.addItemWithTitle_action_keyEquivalent(
            &NSString::from_str(initial_lang.language_menu_title()),
            None,
            &NSString::from_str(""),
        )
    };
    set_item_sf_symbol(&item_lang_menu, "globe", None);

    let lang_submenu = NSMenu::new(mtm);
    let lang_specs = [
        (0isize, Language::En, "English"),
        (1isize, Language::Ru, "Русский"),
        (2isize, Language::Fr, "Français"),
        (3isize, Language::Es, "Español"),
        (4isize, Language::Zh, "简体中文"),
    ];
    let lang_items = lang_specs.map(|(tag, lang_variant, label)| unsafe {
        let item = lang_submenu.addItemWithTitle_action_keyEquivalent(
            &NSString::from_str(label),
            Some(sel!(selectLanguage:)),
            &NSString::from_str(""),
        );
        item.setTag(tag);
        item.setTarget(Some(&handler));
        if lang_variant == initial_lang {
            item.setState(NSControlStateValueOn);
        }
        item
    });
    item_lang_menu.setSubmenu(Some(&lang_submenu));

    // 4. Action Items (Refresh, Updates, System Settings)
    let item_refresh = unsafe {
        menu.addItemWithTitle_action_keyEquivalent(
            &NSString::from_str(initial_lang.refresh_now()),
            Some(sel!(refreshBattery:)),
            &NSString::from_str("r"),
        )
    };
    set_item_sf_symbol(&item_refresh, "arrow.clockwise", None);
    unsafe { item_refresh.setTarget(Some(&handler)) };

    let version_str = initial_lang.check_for_updates(env!("CARGO_PKG_VERSION"));
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
            &NSString::from_str(initial_lang.battery_settings()),
            Some(sel!(openBatterySettings:)),
            &NSString::from_str(","),
        )
    };
    set_item_sf_symbol(&item_settings, "gearshape", Some("gear"));
    unsafe { item_settings.setTarget(Some(&handler)) };

    menu.addItem(&NSMenuItem::separatorItem(mtm));

    // 5. Quit Item
    let item_quit = unsafe {
        menu.addItemWithTitle_action_keyEquivalent(
            &NSString::from_str(initial_lang.quit_app()),
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
        item_display_menu,
        mode_items,
        item_lang_menu,
        lang_items,
        item_refresh,
        item_update,
        item_settings,
        item_quit,
    };

    update_static_menu_labels(&menu_refs, initial_lang, None);

    let app_state = Arc::new(AppState {
        menu_refs: MainThreadBound::new(menu_refs, mtm),
        display_mode: Mutex::new(DisplayMode::Standard),
        language: Mutex::new(initial_lang),
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
            initial_lang,
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
                    let lang = *state.language.lock().unwrap();
                    *state.last_battery_info.lock().unwrap() = Some(info.clone());
                    run_on_main(move |mtm| {
                        let refs = state.menu_refs.get(mtm);
                        update_battery_display(refs, &info, mode, lang, mtm);
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
