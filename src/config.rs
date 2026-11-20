use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    English,
    Japanese,
}

impl Default for Language {
    fn default() -> Self {
        detect_system_language()
    }
}

pub fn detect_system_language() -> Language {
    #[cfg(windows)]
    {
        use windows::Win32::Globalization::GetUserDefaultUILanguage;
        let lang_id = unsafe { GetUserDefaultUILanguage() };
        if (lang_id & 0x03FF) == 0x0011 {
            return Language::Japanese;
        }
    }
    Language::English
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ControlMode {
    #[default]
    Off,
    Limiter,
    Leveler,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DuckRole {
    #[default]
    None,
    Trigger,
    Target,
}

impl DuckRole {
    pub fn next(self) -> Self {
        match self {
            Self::None => Self::Trigger,
            Self::Trigger => Self::Target,
            Self::Target => Self::None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppRule {
    pub mode: ControlMode,
    pub base_volume: f32,
    pub ceiling: f32,
    pub target: f32,
    pub release_ms: f32,
    pub noise_gate: f32,
    pub max_gain: f32,
    pub duck_role: DuckRole,
}

impl Default for AppRule {
    fn default() -> Self {
        Self {
            mode: ControlMode::Off,
            base_volume: 1.0,
            ceiling: 0.75,
            target: 0.55,
            release_ms: 350.0,
            noise_gate: 0.03,
            max_gain: 1.0,
            duck_role: DuckRole::None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DuckingConfig {
    pub trigger_threshold: f32,
    pub reduction: f32,
    pub hold_ms: f32,
    pub release_ms: f32,
}

impl Default for DuckingConfig {
    fn default() -> Self {
        Self {
            trigger_threshold: 0.06,
            reduction: 0.50,
            hold_ms: 500.0,
            release_ms: 400.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub language: Language,
    pub bypass: bool,
    pub auto_start: bool,
    pub ducking: DuckingConfig,
    pub apps: BTreeMap<String, AppRule>,
    pub hidden_apps: BTreeSet<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            language: detect_system_language(),
            bypass: false,
            auto_start: false,
            ducking: DuckingConfig::default(),
            apps: BTreeMap::new(),
            hidden_apps: BTreeSet::new(),
        }
    }
}

impl AppConfig {
    pub fn path() -> Option<PathBuf> {
        ProjectDirs::from("com", "volevel", "Volevel").map(|d| d.config_dir().join("config.toml"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        let Ok(raw) = fs::read_to_string(&path) else {
            return Self::default();
        };
        toml::from_str(&raw).unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = Self::path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(serialized) = toml::to_string_pretty(self) {
            let _ = fs::write(path, serialized);
        }
    }

    pub fn rule_for(&mut self, key: &str, current_volume: f32) -> &mut AppRule {
        self.apps.entry(key.to_string()).or_insert_with(|| AppRule {
            base_volume: current_volume.clamp(0.0, 1.0),
            ..AppRule::default()
        })
    }
}
