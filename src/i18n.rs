use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    En,
    Ru,
    Fr,
    Es,
    Zh,
}

impl Language {
    /// Parses a locale code string (e.g. "ru-RU", "fr_FR.UTF-8", "zh-Hans-CN", "es-ES", "en-US")
    pub fn from_locale_str(locale: &str) -> Option<Self> {
        let cleaned = locale
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_lowercase();
        let prefix = cleaned
            .split(|c: char| c == '-' || c == '_' || c == '.')
            .next()?;

        match prefix {
            "en" => Some(Self::En),
            "ru" => Some(Self::Ru),
            "fr" => Some(Self::Fr),
            "es" => Some(Self::Es),
            "zh" => Some(Self::Zh),
            _ => None,
        }
    }

    /// Detects the active macOS system language from `AppleLanguages` or environment variables
    pub fn detect_system() -> Self {
        // 1. Check macOS Global Domain AppleLanguages (authoritative for .app bundles)
        if let Ok(output) = Command::new("defaults")
            .args(["read", "-g", "AppleLanguages"])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(lang) = Self::parse_apple_languages(&stdout) {
                    return lang;
                }
            }
        }

        // 2. Fallback to POSIX environment variables (LANG, LC_ALL, LC_MESSAGES)
        for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(val) = std::env::var(key) {
                if let Some(lang) = Self::from_locale_str(&val) {
                    return lang;
                }
            }
        }

        // 3. Default to English
        Self::En
    }

    /// Parses the output of `defaults read -g AppleLanguages`
    pub fn parse_apple_languages(output: &str) -> Option<Self> {
        for line in output.lines() {
            let trimmed = line.trim().trim_end_matches(',');
            if trimmed.starts_with('(') || trimmed.starts_with(')') || trimmed.is_empty() {
                continue;
            }
            if let Some(lang) = Self::from_locale_str(trimmed) {
                return Some(lang);
            }
        }
        None
    }

    pub fn battery_title(self) -> &'static str {
        match self {
            Self::En => "Battery",
            Self::Ru => "Аккумулятор",
            Self::Fr => "Batterie",
            Self::Es => "Batería",
            Self::Zh => "电池",
        }
    }

    pub fn calculating(self) -> &'static str {
        match self {
            Self::En => "Calculating time remaining…",
            Self::Ru => "Расчёт оставшегося времени…",
            Self::Fr => "Calcul du temps restant…",
            Self::Es => "Calculando tiempo restante…",
            Self::Zh => "正在计算剩余时间…",
        }
    }

    pub fn fully_charged(self) -> &'static str {
        match self {
            Self::En => "Fully Charged",
            Self::Ru => "Полностью заряжен",
            Self::Fr => "Complètement chargée",
            Self::Es => "Completamente cargada",
            Self::Zh => "已充满",
        }
    }

    pub fn charging_in_progress(self) -> &'static str {
        match self {
            Self::En => "Charging…",
            Self::Ru => "Зарядка…",
            Self::Fr => "En charge…",
            Self::Es => "Cargando…",
            Self::Zh => "正在充电…",
        }
    }

    pub fn plugged_in_not_charging(self) -> &'static str {
        match self {
            Self::En => "Plugged In · Not Charging",
            Self::Ru => "От сети · Не заряжается",
            Self::Fr => "Branché · Pas en charge",
            Self::Es => "Conectado · Sin cargar",
            Self::Zh => "已接通电源 · 未充电",
        }
    }

    pub fn about_remaining(self, time: &str) -> String {
        match self {
            Self::En => format!("About {} remaining", time),
            Self::Ru => format!("Осталось примерно {}", time),
            Self::Fr => format!("Environ {} restantes", time),
            Self::Es => format!("Quedan aprox. {}", time),
            Self::Zh => format!("剩余约 {}", time),
        }
    }

    pub fn until_full(self, time: &str) -> String {
        match self {
            Self::En => format!("{} until fully charged", time),
            Self::Ru => format!("{} до полной зарядки", time),
            Self::Fr => format!("{} avant charge complète", time),
            Self::Es => format!("{} para carga completa", time),
            Self::Zh => format!("充满还需 {}", time),
        }
    }

    pub fn format_short_duration(self, hours: u32, mins: u32) -> String {
        match self {
            Self::Ru => {
                if hours == 0 {
                    format!("{}м", mins)
                } else {
                    format!("{}ч {:02}м", hours, mins)
                }
            }
            Self::En | Self::Fr | Self::Es | Self::Zh => {
                if hours == 0 {
                    format!("{}m", mins)
                } else {
                    format!("{}h {:02}m", hours, mins)
                }
            }
        }
    }

    pub fn power_source_line(self, is_adapter: bool) -> &'static str {
        match (self, is_adapter) {
            (Self::En, false) => "Power Source: Battery",
            (Self::En, true) => "Power Source: Power Adapter",
            (Self::Ru, false) => "Источник питания: Аккумулятор",
            (Self::Ru, true) => "Источник питания: Сетевой адаптер",
            (Self::Fr, false) => "Source : Batterie",
            (Self::Fr, true) => "Source : Adaptateur secteur",
            (Self::Es, false) => "Fuente: Batería",
            (Self::Es, true) => "Fuente: Adaptador de corriente",
            (Self::Zh, false) => "电源：电池",
            (Self::Zh, true) => "电源：电源适配器",
        }
    }

    pub fn battery_health(self, pct: Option<u8>, nom: Option<u32>, des: Option<u32>) -> String {
        match (self, pct, nom, des) {
            (Self::En, Some(p), Some(n), Some(d)) => {
                format!("Battery Health: {}% ({} / {} mAh)", p, n, d)
            }
            (Self::En, Some(p), _, _) => format!("Battery Health: {}%", p),
            (Self::En, None, _, _) => "Battery Health: Normal".to_string(),

            (Self::Ru, Some(p), Some(n), Some(d)) => {
                format!("Здоровье батареи: {}% ({} / {} мА·ч)", p, n, d)
            }
            (Self::Ru, Some(p), _, _) => format!("Здоровье батареи: {}%", p),
            (Self::Ru, None, _, _) => "Здоровье батареи: Нормальное".to_string(),

            (Self::Fr, Some(p), Some(n), Some(d)) => {
                format!("Santé de la batterie : {}% ({} / {} mAh)", p, n, d)
            }
            (Self::Fr, Some(p), _, _) => format!("Santé de la batterie : {}%", p),
            (Self::Fr, None, _, _) => "Santé de la batterie : Normale".to_string(),

            (Self::Es, Some(p), Some(n), Some(d)) => {
                format!("Salud de la batería: {}% ({} / {} mAh)", p, n, d)
            }
            (Self::Es, Some(p), _, _) => format!("Salud de la batería: {}%", p),
            (Self::Es, None, _, _) => "Salud de la batería: Normal".to_string(),

            (Self::Zh, Some(p), Some(n), Some(d)) => {
                format!("电池健康度：{}% ({} / {} mAh)", p, n, d)
            }
            (Self::Zh, Some(p), _, _) => format!("电池健康度：{}%", p),
            (Self::Zh, None, _, _) => "电池健康度：正常".to_string(),
        }
    }

    pub fn cycle_count(self, cycles: Option<u32>) -> String {
        match (self, cycles) {
            (Self::En, Some(c)) => format!("Cycle Count: {} cycles", c),
            (Self::En, None) => "Cycle Count: --".to_string(),
            (Self::Ru, Some(c)) => format!("Количество циклов: {}", c),
            (Self::Ru, None) => "Количество циклов: --".to_string(),
            (Self::Fr, Some(c)) => format!("Nombre de cycles : {}", c),
            (Self::Fr, None) => "Nombre de cycles : --".to_string(),
            (Self::Es, Some(c)) => format!("Ciclos de carga: {}", c),
            (Self::Es, None) => "Ciclos de carga: --".to_string(),
            (Self::Zh, Some(c)) => format!("循环次数：{} 次", c),
            (Self::Zh, None) => "循环次数：--".to_string(),
        }
    }

    pub fn power_metrics(
        self,
        watts: Option<f32>,
        volts: Option<f32>,
        is_charging: bool,
    ) -> String {
        match (self, watts, volts) {
            (Self::En, Some(w), Some(v)) if w > 0.05 => {
                let label = if is_charging {
                    "Charging Rate"
                } else {
                    "Power Draw"
                };
                format!("{}: {:.1} W ({:.1} V)", label, w, v)
            }
            (Self::En, _, Some(v)) => format!("Voltage: {:.2} V", v),
            (Self::En, _, None) => "Power Draw: --".to_string(),

            (Self::Ru, Some(w), Some(v)) if w > 0.05 => {
                let label = if is_charging {
                    "Мощность зарядки"
                } else {
                    "Потребление"
                };
                format!("{}: {:.1} Вт ({:.1} В)", label, w, v)
            }
            (Self::Ru, _, Some(v)) => format!("Напряжение: {:.2} В", v),
            (Self::Ru, _, None) => "Потребление: --".to_string(),

            (Self::Fr, Some(w), Some(v)) if w > 0.05 => {
                let label = if is_charging {
                    "Puissance de charge"
                } else {
                    "Consommation"
                };
                format!("{} : {:.1} W ({:.1} V)", label, w, v)
            }
            (Self::Fr, _, Some(v)) => format!("Tension : {:.2} V", v),
            (Self::Fr, _, None) => "Consommation : --".to_string(),

            (Self::Es, Some(w), Some(v)) if w > 0.05 => {
                let label = if is_charging {
                    "Potencia de carga"
                } else {
                    "Consumo"
                };
                format!("{}: {:.1} W ({:.1} V)", label, w, v)
            }
            (Self::Es, _, Some(v)) => format!("Voltaje: {:.2} V", v),
            (Self::Es, _, None) => "Consumo: --".to_string(),

            (Self::Zh, Some(w), Some(v)) if w > 0.05 => {
                let label = if is_charging {
                    "充电功率"
                } else {
                    "当前功耗"
                };
                format!("{}：{:.1} W ({:.1} V)", label, w, v)
            }
            (Self::Zh, _, Some(v)) => format!("电压：{:.2} V", v),
            (Self::Zh, _, None) => "当前功耗：--".to_string(),
        }
    }

    pub fn menu_bar_display(self) -> &'static str {
        match self {
            Self::En => "Menu Bar Display",
            Self::Ru => "Вид в строке меню",
            Self::Fr => "Affichage barre des menus",
            Self::Es => "Visualización en barra",
            Self::Zh => "菜单栏显示格式",
        }
    }

    pub fn display_mode_labels(self) -> [&'static str; 4] {
        match self {
            Self::En => [
                "Time & Percentage (3h 15m · 76%)",
                "Compact (3:15 · 76%)",
                "Time Remaining Only (3h 15m)",
                "Percentage Only (76%)",
            ],
            Self::Ru => [
                "Время и процент (3ч 15м · 76%)",
                "Компактный (3:15 · 76%)",
                "Только оставшееся время (3ч 15м)",
                "Только процент (76%)",
            ],
            Self::Fr => [
                "Temps et pourcentage (3h 15m · 76%)",
                "Compact (3:15 · 76%)",
                "Temps restant uniquement (3h 15m)",
                "Pourcentage uniquement (76%)",
            ],
            Self::Es => [
                "Tiempo y porcentaje (3h 15m · 76%)",
                "Compacto (3:15 · 76%)",
                "Solo tiempo restante (3h 15m)",
                "Solo porcentaje (76%)",
            ],
            Self::Zh => [
                "时间与百分比 (3h 15m · 76%)",
                "紧凑模式 (3:15 · 76%)",
                "仅显示剩余时间 (3h 15m)",
                "仅显示百分比 (76%)",
            ],
        }
    }

    pub fn language_menu_title(self) -> &'static str {
        match self {
            Self::En => "Language",
            Self::Ru => "Язык (Language)",
            Self::Fr => "Langue (Language)",
            Self::Es => "Idioma (Language)",
            Self::Zh => "语言 (Language)",
        }
    }

    pub fn refresh_now(self) -> &'static str {
        match self {
            Self::En => "Refresh Now",
            Self::Ru => "Обновить сейчас",
            Self::Fr => "Actualiser maintenant",
            Self::Es => "Actualizar ahora",
            Self::Zh => "立即刷新",
        }
    }

    pub fn check_for_updates(self, ver: &str) -> String {
        match self {
            Self::En => format!("Check for Updates… (v{})", ver),
            Self::Ru => format!("Проверить обновления… (v{})", ver),
            Self::Fr => format!("Rechercher des mises à jour… (v{})", ver),
            Self::Es => format!("Buscar actualizaciones… (v{})", ver),
            Self::Zh => format!("检查更新… (v{})", ver),
        }
    }

    pub fn checking_updates(self) -> &'static str {
        match self {
            Self::En => "Checking for Updates…",
            Self::Ru => "Проверка обновлений…",
            Self::Fr => "Recherche de mises à jour…",
            Self::Es => "Buscando actualizaciones…",
            Self::Zh => "正在检查更新…",
        }
    }

    pub fn update_available(self, ver: &str) -> String {
        match self {
            Self::En => format!("Update Available (v{}) — Download…", ver),
            Self::Ru => format!("Доступно обновление (v{}) — Скачать…", ver),
            Self::Fr => format!("Mise à jour disponible (v{}) — Télécharger…", ver),
            Self::Es => format!("Actualización disponible (v{}) — Descargar…", ver),
            Self::Zh => format!("发现新版本 (v{}) — 点击下载…", ver),
        }
    }

    pub fn up_to_date(self, ver: &str) -> String {
        match self {
            Self::En => format!("Up to Date (v{})", ver),
            Self::Ru => format!("Актуальная версия (v{})", ver),
            Self::Fr => format!("À jour (v{})", ver),
            Self::Es => format!("Actualizado (v{})", ver),
            Self::Zh => format!("已是最新版本 (v{})", ver),
        }
    }

    pub fn update_failed_offline(self) -> &'static str {
        match self {
            Self::En => "Update Check Failed (Offline)",
            Self::Ru => "Ошибка проверки (Нет сети)",
            Self::Fr => "Échec de la vérification (Hors ligne)",
            Self::Es => "Error al buscar (Sin conexión)",
            Self::Zh => "检查更新失败（离线）",
        }
    }

    pub fn battery_settings(self) -> &'static str {
        match self {
            Self::En => "Battery Settings…",
            Self::Ru => "Настройки аккумулятора…",
            Self::Fr => "Réglages Batterie…",
            Self::Es => "Ajustes de Batería…",
            Self::Zh => "电池设置…",
        }
    }

    pub fn quit_app(self) -> &'static str {
        match self {
            Self::En => "Quit BatteryBar",
            Self::Ru => "Завершить BatteryBar",
            Self::Fr => "Quitter BatteryBar",
            Self::Es => "Salir de BatteryBar",
            Self::Zh => "退出 BatteryBar",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_locale_parsing() {
        assert_eq!(Language::from_locale_str("en-US"), Some(Language::En));
        assert_eq!(Language::from_locale_str("ru-RU"), Some(Language::Ru));
        assert_eq!(Language::from_locale_str("fr_FR.UTF-8"), Some(Language::Fr));
        assert_eq!(Language::from_locale_str("es-419"), Some(Language::Es));
        assert_eq!(Language::from_locale_str("zh-Hans-CN"), Some(Language::Zh));
        assert_eq!(Language::from_locale_str("de-DE"), None);
    }

    #[test]
    fn test_parse_apple_languages() {
        let sample_ru = "(\n    \"ru-RU\",\n    \"en-RU\"\n)";
        assert_eq!(Language::parse_apple_languages(sample_ru), Some(Language::Ru));

        let sample_zh = "(\n    \"zh-Hans-CN\",\n    \"en-US\"\n)";
        assert_eq!(Language::parse_apple_languages(sample_zh), Some(Language::Zh));

        let sample_fallback = "(\n    \"de-DE\",\n    \"fr-FR\"\n)";
        assert_eq!(
            Language::parse_apple_languages(sample_fallback),
            Some(Language::Fr)
        );
    }

    #[test]
    fn test_translations_present_for_all_languages() {
        for lang in [
            Language::En,
            Language::Ru,
            Language::Fr,
            Language::Es,
            Language::Zh,
        ] {
            assert!(!lang.battery_title().is_empty());
            assert!(!lang.calculating().is_empty());
            assert!(!lang.about_remaining("3h 15m").is_empty());
            assert!(!lang.battery_health(Some(85), Some(3500), Some(4100)).is_empty());
            assert!(!lang.quit_app().is_empty());
        }
    }
}
