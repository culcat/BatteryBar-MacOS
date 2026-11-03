use regex::Regex;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatteryState {
    Discharging,
    Charging,
    Charged,
    AcAttached,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatteryInfo {
    pub percentage: u8,
    pub state: BatteryState,
    pub remaining_time: Option<String>, // e.g. "3:15"
    pub power_source: String,          // e.g. "Battery Power" or "AC Power"
}

impl BatteryInfo {
    /// Formats the string to be displayed in the macOS menu bar
    pub fn menu_bar_text(&self) -> String {
        match self.state {
            BatteryState::Discharging => match &self.remaining_time {
                Some(time) => format!("🔋 {} ({}%)", time, self.percentage),
                None => format!("🔋 {}% (calc...)", self.percentage),
            },
            BatteryState::Charging => match &self.remaining_time {
                Some(time) => format!("⚡ {} ({}%)", time, self.percentage),
                None => format!("⚡ {}% (charging)", self.percentage),
            },
            BatteryState::Charged => format!("⚡ 100% (Charged)"),
            BatteryState::AcAttached => format!("🔌 {}% (AC)", self.percentage),
            BatteryState::Unknown(_) => format!("🔋 {}%", self.percentage),
        }
    }
}

/// Executes `pmset -g batt` in terminal and parses the output
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
    parse_pmset_output(&stdout)
}

/// Parses output from `pmset -g batt`
pub fn parse_pmset_output(output: &str) -> Result<BatteryInfo, String> {
    // Determine power source (e.g. 'Battery Power' or 'AC Power')
    let power_source = if output.contains("'Battery Power'") {
        "Battery Power".to_string()
    } else if output.contains("'AC Power'") {
        "AC Power".to_string()
    } else {
        "Unknown Power".to_string()
    };

    // Regex to match: `76%; discharging; 3:15 remaining`
    // or `100%; charged; 0:00 remaining`
    // or `76%; discharging; (no estimate)`
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
        })
    } else {
        Err(format!("Could not parse pmset output: {}", output))
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
        assert_eq!(info.power_source, "Battery Power");
        assert_eq!(info.menu_bar_text(), "🔋 3:15 (76%)");
    }

    #[test]
    fn test_parse_discharging_no_estimate() {
        let raw = "Now drawing from 'Battery Power'\n -InternalBattery-0 (id=22741091)\t82%; discharging; (no estimate) present: true";
        let info = parse_pmset_output(raw).unwrap();
        assert_eq!(info.percentage, 82);
        assert_eq!(info.state, BatteryState::Discharging);
        assert_eq!(info.remaining_time, None);
        assert_eq!(info.menu_bar_text(), "🔋 82% (calc...)");
    }

    #[test]
    fn test_parse_charging() {
        let raw = "Now drawing from 'AC Power'\n -InternalBattery-0 (id=22741091)\t45%; charging; 1:20 remaining present: true";
        let info = parse_pmset_output(raw).unwrap();
        assert_eq!(info.percentage, 45);
        assert_eq!(info.state, BatteryState::Charging);
        assert_eq!(info.remaining_time, Some("1:20".to_string()));
        assert_eq!(info.power_source, "AC Power");
        assert_eq!(info.menu_bar_text(), "⚡ 1:20 (45%)");
    }

    #[test]
    fn test_parse_charged() {
        let raw = "Now drawing from 'AC Power'\n -InternalBattery-0 (id=22741091)\t100%; charged; 0:00 remaining present: true";
        let info = parse_pmset_output(raw).unwrap();
        assert_eq!(info.percentage, 100);
        assert_eq!(info.state, BatteryState::Charged);
        assert_eq!(info.menu_bar_text(), "⚡ 100% (Charged)");
    }
}
