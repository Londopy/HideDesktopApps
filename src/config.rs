use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub hotkeys: HotkeysConfig,
    pub startup: StartupConfig,
    pub defaults: DefaultsConfig,
    pub updater: UpdaterConfig,
    pub notifications: NotificationsConfig,
    pub discord: DiscordConfig,
    pub window_filter: WindowFilterConfig,
    #[serde(default)]
    pub profiles: Vec<ProfileConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeysConfig {
    pub icons: String,
    pub taskbar: String,
    pub windows: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupConfig {
    pub enabled: bool,
    pub delay_s: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultsConfig {
    pub profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdaterConfig {
    pub enabled: bool,
    pub channel: String,
    pub check_interval_h: u32,
    pub last_checked: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationsConfig {
    pub enabled: bool,
    pub on_update: bool,
    pub on_hotkey_fail: bool,
    pub on_profile_switch: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordConfig {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowFilterConfig {
    pub exclude_processes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    pub name: String,
    pub hotkey: String,
    pub icons: bool,
    pub taskbar: bool,
    pub windows: bool,
}

impl Default for HotkeysConfig {
    fn default() -> Self {
        Self {
            icons: "ctrl+alt+h".to_string(),
            taskbar: "ctrl+alt+t".to_string(),
            windows: "ctrl+alt+w".to_string(),
        }
    }
}

impl Default for StartupConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            delay_s: 30,
        }
    }
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            profile: String::new(),
        }
    }
}

impl Default for UpdaterConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            channel: "stable".to_string(),
            check_interval_h: 24,
            last_checked: String::new(),
        }
    }
}

impl Default for NotificationsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            on_update: true,
            on_hotkey_fail: true,
            on_profile_switch: false,
        }
    }
}

impl Default for DiscordConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl Default for WindowFilterConfig {
    fn default() -> Self {
        Self {
            // Processes whose windows should never be hidden —
            // password managers and system processes you don't want disappearing.
            exclude_processes: vec![
                "1password".to_string(),
                "taskmgr".to_string(),
                "winlogon".to_string(),
            ],
        }
    }
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            name: "Default".to_string(),
            hotkey: String::new(),
            icons: true,
            taskbar: false,
            windows: false,
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            hotkeys: HotkeysConfig::default(),
            startup: StartupConfig::default(),
            defaults: DefaultsConfig::default(),
            updater: UpdaterConfig::default(),
            notifications: NotificationsConfig::default(),
            discord: DiscordConfig::default(),
            window_filter: WindowFilterConfig::default(),
            profiles: default_profiles(),
        }
    }
}

/// Three built-in profiles included on first run.
fn default_profiles() -> Vec<ProfileConfig> {
    vec![
        // Focus: hide icons + taskbar, leave windows alone
        ProfileConfig {
            name: "Focus".to_string(),
            hotkey: "ctrl+alt+f".to_string(),
            icons: true,
            taskbar: true,
            windows: false,
        },
        // Presentation: hide everything
        ProfileConfig {
            name: "Presentation".to_string(),
            hotkey: "ctrl+alt+p".to_string(),
            icons: true,
            taskbar: true,
            windows: true,
        },
        // Clean Desktop: hide icons only, keep taskbar visible
        ProfileConfig {
            name: "Clean Desktop".to_string(),
            hotkey: String::new(),
            icons: true,
            taskbar: false,
            windows: false,
        },
    ]
}

pub fn config_dir() -> Result<PathBuf> {
    // Check for portable mode: if a file named "portable" exists next to the exe
    if let Ok(exe) = std::env::current_exe() {
        let portable_marker = exe.parent().unwrap_or(std::path::Path::new(".")).join("portable");
        if portable_marker.exists() {
            return Ok(exe.parent().unwrap_or(std::path::Path::new(".")).to_path_buf());
        }
    }

    let appdata = std::env::var("APPDATA").context("APPDATA env var not set")?;
    let dir = PathBuf::from(appdata).join("HideDesktopApps");
    std::fs::create_dir_all(&dir).context("Failed to create config dir")?;
    Ok(dir)
}

pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.toml"))
}

/// Parse a legacy config.ini from the Python version and return an AppConfig.
/// Also removes the old VBS startup launcher if it's in the same directory.
fn migrate_ini(ini_path: &std::path::Path, dir: &std::path::Path) -> Result<AppConfig> {
    let content = std::fs::read_to_string(ini_path)?;
    let mut config = AppConfig::default();
    let mut icons_hidden_on_start = false;
    let mut log_lines: Vec<String> = Vec::new();

    log_lines.push("Migration from config.ini".to_string());

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.starts_with(';') || line.is_empty() || line.starts_with('[') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            // The Python ini used snake_case keys inside [Settings] etc.
            let key = key.trim().to_lowercase();
            let key = key.trim_start_matches("settings.").trim_start_matches("hotkeys.");
            let value = value.trim();
            match key {
                "hotkey_icons" | "icons" => {
                    config.hotkeys.icons = value.to_string();
                    log_lines.push(format!("  icons hotkey: {value}"));
                }
                "hotkey_taskbar" | "taskbar" => {
                    config.hotkeys.taskbar = value.to_string();
                    log_lines.push(format!("  taskbar hotkey: {value}"));
                }
                "hotkey_windows" | "windows" => {
                    config.hotkeys.windows = value.to_string();
                    log_lines.push(format!("  windows hotkey: {value}"));
                }
                "startup_enabled" | "run_on_startup" => {
                    config.startup.enabled = value.eq_ignore_ascii_case("true") || value == "1";
                    log_lines.push(format!("  startup: {}", config.startup.enabled));
                }
                "startup_delay" | "startup_delay_s" => {
                    if let Ok(d) = value.parse::<u32>() {
                        config.startup.delay_s = d;
                        log_lines.push(format!("  startup delay: {d}s"));
                    }
                }
                "updater_enabled" | "auto_update" => {
                    config.updater.enabled = value.eq_ignore_ascii_case("true") || value == "1";
                }
                "discord_enabled" | "discord" => {
                    config.discord.enabled = value.eq_ignore_ascii_case("true") || value == "1";
                }
                "icons_hidden" => {
                    // Python version could persist state; create a "Launch Hidden" profile.
                    icons_hidden_on_start = value.eq_ignore_ascii_case("true") || value == "1";
                }
                _ => {}
            }
        }
    }

    // If the old config had icons hidden at startup, create a matching profile.
    if icons_hidden_on_start {
        config.profiles.push(ProfileConfig {
            name: "Launch Hidden".to_string(),
            hotkey: String::new(),
            icons: true,
            taskbar: false,
            windows: false,
        });
        config.defaults.profile = "Launch Hidden".to_string();
        log_lines.push("  created 'Launch Hidden' profile (icons_hidden was true)".to_string());
    }

    // Remove the old VBS startup launcher if it exists next to the ini.
    let vbs = dir.join("HideDesktopApps.vbs");
    if vbs.exists() {
        if let Err(e) = std::fs::remove_file(&vbs) {
            log_lines.push(format!("  WARNING: could not remove VBS launcher: {e}"));
        } else {
            log_lines.push("  removed HideDesktopApps.vbs".to_string());
        }
    }

    // Write a migration log alongside the config.
    let log_path = dir.join("migration.log");
    let _ = std::fs::write(&log_path, log_lines.join("\n") + "\n");

    Ok(config)
}

pub fn load_config() -> Result<AppConfig> {
    let path = config_path()?;

    if !path.exists() {
        // Check for legacy config.ini to migrate
        let dir = config_dir()?;
        let ini_path = dir.join("config.ini");
        if ini_path.exists() {
            eprintln!("Migrating config.ini to config.toml");
            match migrate_ini(&ini_path, &dir) {
                Ok(cfg) => {
                    save_config(&cfg)?;
                    // Rename so migration doesn't run again
                    let _ = std::fs::rename(&ini_path, dir.join("config.ini.migrated"));
                    return Ok(cfg);
                }
                Err(e) => {
                    eprintln!("Migration failed: {e}, using defaults");
                }
            }
        }

        let default = AppConfig::default();
        save_config(&default)?;
        return Ok(default);
    }

    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Reading config from {}", path.display()))?;

    let config: AppConfig = toml::from_str(&content)
        .with_context(|| "Parsing config.toml")?;

    Ok(config)
}

pub fn save_config(config: &AppConfig) -> Result<()> {
    let path = config_path()?;
    let content = toml::to_string_pretty(config).context("Serializing config")?;
    std::fs::write(&path, content)
        .with_context(|| format!("Writing config to {}", path.display()))?;
    Ok(())
}
