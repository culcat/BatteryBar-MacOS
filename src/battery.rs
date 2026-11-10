use crate::i18n::Language;
use regex::Regex;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    Standard,       // "3h 15m · 76%"
    Compact,        // "3:15 · 76%"
    TimeOnly,       // "3h 15m"
    PercentageOnly, // "76%"
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatteryState {
    Discharging,
    Charging,
    Charged,
    AcAttached,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct BatteryInfo {
    pub percentage: u8,
    pub state: BatteryState,
    pub remaining_time: Option<String>, // e.g. "3:15"
    pub power_source: String,          // e.g. "Battery" or "Power Adapter"
    pub cycle_count: Option<u32>,
    pub health_percent: Option<u8>,
    pub nominal_capacity_mah: Option<u32>,
    pub design_capacity_mah: Option<u32>,
    pub power_watts: Option<f32>,
    pub voltage_volts: Option<f32>,
}

impl BatteryInfo {
    /// Converts raw "3:15" into localized human-readable macOS format (e.g., "3h 15m" or "3ч 15м")
    pub fn formatted_time_localized(&self, lang: Language) -> Option<String> {
        let raw = self.remaining_time.as_ref()?;
        let mut parts = raw.split(':');
        let hours: u32 = parts.next()?.parse().ok()?;
        let mins: u32 = parts.next()?.parse().ok()?;

        if hours == 0 && mins == 0 && self.state == BatteryState::Charged {
            return None;
        }
        Some(lang.format_short_duration(hours, mins))
    }

    /// Converts raw "3:15" into human-readable macOS format "3h 15m" (or "45m" if hours == 0)
    #[allow(dead_code)]
    pub fn formatted_time(&self) -> Option<String> {
        self.formatted_time_localized(Language::En)
    }

    /// Formats the localized title string for the macOS menu bar button (paired with an SF Symbol icon)
    pub fn menu_bar_title_localized(&self, mode: DisplayMode, lang: Language) -> String {
        match mode {
            DisplayMode::PercentageOnly => format!("{}%", self.percentage),
            DisplayMode::TimeOnly => match self.formatted_time_localized(lang) {
                Some(t) => t,
                None => format!("{}%", self.percentage),
            },
            DisplayMode::Compact => match (&self.state, &self.remaining_time) {
                (BatteryState::Charged, _) => format!("{}%", self.percentage),
                (_, Some(raw)) if raw != "0:00" => format!("{} · {}%", raw, self.percentage),
                _ => format!("{}%", self.percentage),
            },
            DisplayMode::Standard => match (&self.state, self.formatted_time_localized(lang)) {
                (BatteryState::Charged, _) => format!("{}%", self.percentage),
                (_, Some(human_time)) => format!("{} · {}%", human_time, self.percentage),
                _ => format!("{}%", self.percentage),
            },
        }
    }

    /// Formats the title string for the macOS menu bar button (English default)
    #[allow(dead_code)]
    pub fn menu_bar_title(&self, mode: DisplayMode) -> String {
        self.menu_bar_title_localized(mode, Language::En)
    }

    /// Legacy compatibility helper used in basic tests
    #[allow(dead_code)]
    pub fn menu_bar_text(&self) -> String {
        self.menu_bar_title(DisplayMode::Standard)
    }

    /// Primary SF Symbol name (macOS 13/14/15+)
    pub fn sf_symbol_name(&self) -> &'static str {
        match self.state {
            BatteryState::Charging => "battery.100percent.bolt",
            BatteryState::Charged | BatteryState::AcAttached => "powerplug.fill",
            BatteryState::Discharging | BatteryState::Unknown(_) => {
                if self.percentage >= 88 {
                    "battery.100percent"
                } else if self.percentage >= 63 {
                    "battery.75percent"
                } else if self.percentage >= 38 {
                    "battery.50percent"
                } else if self.percentage >= 13 {
                    "battery.25percent"
                } else {
                    "battery.0percent"
                }
            }
        }
    }

    /// Fallback SF Symbol name (macOS 11/12)
    pub fn fallback_sf_symbol_name(&self) -> &'static str {
        match self.state {
            BatteryState::Charging => "battery.100.bolt",
            BatteryState::Charged | BatteryState::AcAttached => "powerplug.fill",
            BatteryState::Discharging | BatteryState::Unknown(_) => {
                if self.percentage >= 88 {
                    "battery.100"
                } else if self.percentage >= 63 {
                    "battery.75"
                } else if self.percentage >= 38 {
                    "battery.50"
                } else if self.percentage >= 13 {
                    "battery.25"
                } else {
                    "battery.0"
                }
            }
        }
    }

    /// Primary localized status line for the native header card (Control Center style)
    pub fn primary_status_line_localized(&self, lang: Language) -> String {
        match self.state {
            BatteryState::Discharging => match self.formatted_time_localized(lang) {
                Some(t) => lang.about_remaining(&t),
                None => lang.calculating().to_string(),
            },
            BatteryState::Charging => match self.formatted_time_localized(lang) {
                Some(t) => lang.until_full(&t),
                None => lang.charging_in_progress().to_string(),
            },
            BatteryState::Charged => lang.fully_charged().to_string(),
            BatteryState::AcAttached => lang.plugged_in_not_charging().to_string(),
            BatteryState::Unknown(ref s) => format!("Status: {}", s),
        }
    }

    /// Primary status line for the native header card (English default)
    #[allow(dead_code)]
    pub fn primary_status_line(&self) -> String {
        self.primary_status_line_localized(Language::En)
    }

    /// Secondary localized power source line for the native header card
    pub fn source_line_localized(&self, lang: Language) -> String {
        let is_adapter = self.power_source == "Power Adapter";
        lang.power_source_line(is_adapter).to_string()
    }

    /// Secondary power source line for the native header card (English default)
    #[allow(dead_code)]
    pub fn source_line(&self) -> String {
        self.source_line_localized(Language::En)
    }
}

/// Executes `pmset -g batt` and `ioreg -rc AppleSmartBattery -d 1` to gather battery telemetry
pub fn fetch_battery_info() -> Result<BatteryInfo, String> {
    let output = Command::new("pmset")
        .arg("-g")
        .arg("batt")
        .output()
        .map_err(|e| format!("Failed to execute pmset: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "pmset exited with code {:?}",
            output.status.code()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut info = parse_pmset_output(&stdout)?;

    // Enrich with hardware metrics from ioreg (takes ~3-4ms)
    if let Ok(ioreg_out) = Command::new("ioreg")
        .args(["-rc", "AppleSmartBattery", "-d", "1"])
        .output()
    {
        if ioreg_out.status.success() {
            let ioreg_str = String::from_utf8_lossy(&ioreg_out.stdout);
            enrich_with_ioreg(&mut info, &ioreg_str);
        }
    }

    Ok(info)
}

/// Parses output from `pmset -g batt`
pub fn parse_pmset_output(output: &str) -> Result<BatteryInfo, String> {
    let power_source = if output.contains("'Battery Power'") {
        "Battery".to_string()
    } else if output.contains("'AC Power'") {
        "Power Adapter".to_string()
    } else {
        "Battery".to_string()
    };

    let re = Regex::new(r"(\d+)%;\s*([^;]+);(?:\s*(?:(\d+:\d+)\s*remaining|\(no estimate\)))?")
        .map_err(|e| e.to_string())?;

    if let Some(caps) = re.captures(output) {
        let percentage = caps
            .get(1)
            .and_then(|m| m.as_str().parse::<u8>().ok())
            .unwrap_or(0);

        let status_str = caps
            .get(2)
            .map(|m| m.as_str().trim().to_lowercase())
            .unwrap_or_default();

        let remaining_time = caps.get(3).map(|m| m.as_str().to_string());

        let state = match status_str.as_str() {
            "discharging" => BatteryState::Discharging,
            "charging" => BatteryState::Charging,
            "charged" | "finishing charge" => BatteryState::Charged,
            "ac attached" => BatteryState::AcAttached,
            other => BatteryState::Unknown(other.to_string()),
        };

        Ok(BatteryInfo {
            percentage,
            state,
            remaining_time,
            power_source,
            cycle_count: None,
            health_percent: None,
            nominal_capacity_mah: None,
            design_capacity_mah: None,
            power_watts: None,
            voltage_volts: None,
        })
    } else {
        Err(format!("Could not parse pmset output: {}", output))
    }
}

/// Extracts CycleCount, NominalChargeCapacity, DesignCapacity, Voltage, and InstantAmperage from ioreg
pub fn enrich_with_ioreg(info: &mut BatteryInfo, ioreg_output: &str) {
    let extract_u64 = |pattern: &str| -> Option<u64> {
        let re = Regex::new(pattern).ok()?;
        let caps = re.captures(ioreg_output)?;
        caps.get(1)?.as_str().parse::<u64>().ok()
    };

    if let Some(cycles) = extract_u64(r#""CycleCount"\s*=\s*(\d+)"#) {
        info.cycle_count = Some(cycles as u32);
    }

    let nominal = extract_u64(r#""NominalChargeCapacity"\s*=\s*(\d+)"#)
        .or_else(|| extract_u64(r#""FullChargeCapacity"\s*=\s*(\d+)"#));
    let design = extract_u64(r#""DesignCapacity"\s*=\s*(\d+)"#);

    if let (Some(nom), Some(des)) = (nominal, design) {
        if des > 0 {
            info.nominal_capacity_mah = Some(nom as u32);
            info.design_capacity_mah = Some(des as u32);
            let pct = ((nom as f64 / des as f64) * 100.0).round() as u8;
            info.health_percent = Some(pct.min(100));
        }
    }

    let voltage_mv = extract_u64(r#""Voltage"\s*=\s*(\d+)"#);
    let amperage_raw = extract_u64(r#""InstantAmperage"\s*=\s*(\d+)"#);

    if let Some(v_mv) = voltage_mv {
        let v_volts = v_mv as f32 / 1000.0;
        info.voltage_volts = Some(v_volts);

        if let Some(amp_u64) = amperage_raw {
            // InstantAmperage is reported as unsigned representation of signed 64-bit integer in mA
            let amp_ma = amp_u64 as i64;
            let watts = (v_volts * (amp_ma.abs() as f32 / 1000.0)).abs();
            if watts < 250.0 {
                info.power_watts = Some(watts);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_discharging_with_estimate() {
        let raw = "Now drawing from 'Battery Power'\n -InternalBattery-0 (id=22741091)\t76%; discharging; 3:15 remaining present: true";
        let info = parse_pmset_output(raw).unwrap();
        assert_eq!(info.percentage, 76);
        assert_eq!(info.state, BatteryState::Discharging);
        assert_eq!(info.remaining_time, Some("3:15".to_string()));
        assert_eq!(info.power_source, "Battery");
        assert_eq!(info.formatted_time(), Some("3h 15m".to_string()));
        assert_eq!(info.menu_bar_title(DisplayMode::Standard), "3h 15m · 76%");
        assert_eq!(info.menu_bar_title(DisplayMode::Compact), "3:15 · 76%");
        assert_eq!(info.menu_bar_title(DisplayMode::TimeOnly), "3h 15m");
        assert_eq!(info.menu_bar_title(DisplayMode::PercentageOnly), "76%");
    }

    #[test]
    fn test_parse_discharging_no_estimate() {
        let raw = "Now drawing from 'Battery Power'\n -InternalBattery-0 (id=22741091)\t82%; discharging; (no estimate) present: true";
        let info = parse_pmset_output(raw).unwrap();
        assert_eq!(info.percentage, 82);
        assert_eq!(info.state, BatteryState::Discharging);
        assert_eq!(info.remaining_time, None);
        assert_eq!(info.menu_bar_title(DisplayMode::Standard), "82%");
        assert_eq!(info.primary_status_line(), "Calculating time remaining…");
    }

    #[test]
    fn test_parse_charging() {
        let raw = "Now drawing from 'AC Power'\n -InternalBattery-0 (id=22741091)\t45%; charging; 0:45 remaining present: true";
        let info = parse_pmset_output(raw).unwrap();
        assert_eq!(info.percentage, 45);
        assert_eq!(info.state, BatteryState::Charging);
        assert_eq!(info.formatted_time(), Some("45m".to_string()));
        assert_eq!(info.power_source, "Power Adapter");
        assert_eq!(info.menu_bar_title(DisplayMode::Standard), "45m · 45%");
        assert_eq!(info.primary_status_line(), "45m until fully charged");
    }

    #[test]
    fn test_parse_charged() {
        let raw = "Now drawing from 'AC Power'\n -InternalBattery-0 (id=22741091)\t100%; charged; 0:00 remaining present: true";
        let info = parse_pmset_output(raw).unwrap();
        assert_eq!(info.percentage, 100);
        assert_eq!(info.state, BatteryState::Charged);
        assert_eq!(info.menu_bar_title(DisplayMode::Standard), "100%");
        assert_eq!(info.primary_status_line(), "Fully Charged");
    }

    #[test]
    fn test_enrich_with_ioreg() {
        let raw = "Now drawing from 'Battery Power'\n -InternalBattery-0 (id=22741091)\t76%; discharging; 3:15 remaining present: true";
        let mut info = parse_pmset_output(raw).unwrap();
        let sample_ioreg = r#"
            "Voltage" = 11155
            "CycleCount" = 446
            "BatteryData" = {"FullChargeCapacity"=3445,"NominalChargeCapacity"=3575,"DesignCapacity"=4382}
            "InstantAmperage" = 18446744073709550616
        "#;
        enrich_with_ioreg(&mut info, sample_ioreg);
        assert_eq!(info.cycle_count, Some(446));
        assert_eq!(info.health_percent, Some(82));
        assert!(info.power_watts.unwrap() > 10.0);
    }
}
