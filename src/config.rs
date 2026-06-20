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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
            // never hide these — password managers and system stuff
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

// the 3 default profiles on first run
fn default_profiles() -> Vec<ProfileConfig> {
    vec![
        // hide icons + taskbar, leave windows alone
        ProfileConfig {
            name: "Focus".to_string(),
            hotkey: "ctrl+alt+f".to_string(),
            icons: true,
            taskbar: true,
            windows: false,
        },
        // hide everything
        ProfileConfig {
            name: "Presentation".to_string(),
            hotkey: "ctrl+alt+p".to_string(),
            icons: true,
            taskbar: true,
            windows: true,
        },
        // just icons
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
    // portable mode: if there's a "portable" file next to the exe, use that folder
    if let Ok(exe) = std::env::current_exe() {
        let portable_marker = exe
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join("portable");
        if portable_marker.exists() {
            return Ok(exe
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .to_path_buf());
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

// Pure parser for the old python config.ini: turns its text into an AppConfig
// plus a flag for whether icons were set to start hidden. No file I/O so it can
// be unit-tested. `migrate_ini` wraps this with logging and cleanup.
fn parse_ini(content: &str) -> (AppConfig, bool) {
    let mut config = AppConfig::default();
    let mut icons_hidden_on_start = false;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#')
            || line.starts_with(';')
            || line.is_empty()
            || line.starts_with('[')
        {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            // old python version used snake_case keys inside [Settings] etc.
            let key = key.trim().to_lowercase();
            let key = key
                .trim_start_matches("settings.")
                .trim_start_matches("hotkeys.");
            let value = value.trim();
            match key {
                "hotkey_icons" | "icons" => config.hotkeys.icons = value.to_string(),
                "hotkey_taskbar" | "taskbar" => config.hotkeys.taskbar = value.to_string(),
                "hotkey_windows" | "windows" => config.hotkeys.windows = value.to_string(),
                "startup_enabled" | "run_on_startup" => {
                    config.startup.enabled = value.eq_ignore_ascii_case("true") || value == "1";
                }
                "startup_delay" | "startup_delay_s" => {
                    if let Ok(d) = value.parse::<u32>() {
                        config.startup.delay_s = d;
                    }
                }
                "updater_enabled" | "auto_update" => {
                    config.updater.enabled = value.eq_ignore_ascii_case("true") || value == "1";
                }
                "discord_enabled" | "discord" => {
                    config.discord.enabled = value.eq_ignore_ascii_case("true") || value == "1";
                }
                "icons_hidden" => {
                    // old python version could save icons state, carry it forward
                    icons_hidden_on_start = value.eq_ignore_ascii_case("true") || value == "1";
                }
                _ => {}
            }
        }
    }

    (config, icons_hidden_on_start)
}

// migrates old config.ini from the python version of the app
fn migrate_ini(ini_path: &std::path::Path, dir: &std::path::Path) -> Result<AppConfig> {
    let content = std::fs::read_to_string(ini_path)?;
    let (mut config, icons_hidden_on_start) = parse_ini(&content);

    let mut log_lines: Vec<String> = vec!["Migration from config.ini".to_string()];
    log_lines.push(format!("  icons hotkey: {}", config.hotkeys.icons));
    log_lines.push(format!("  taskbar hotkey: {}", config.hotkeys.taskbar));
    log_lines.push(format!("  windows hotkey: {}", config.hotkeys.windows));
    log_lines.push(format!("  startup: {}", config.startup.enabled));
    log_lines.push(format!("  startup delay: {}s", config.startup.delay_s));

    // if it was set to start with icons hidden, create a profile for that
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

    // delete the old vbs launcher if it's still there
    let vbs = dir.join("HideDesktopApps.vbs");
    if vbs.exists() {
        if let Err(e) = std::fs::remove_file(&vbs) {
            log_lines.push(format!("  WARNING: could not remove VBS launcher: {e}"));
        } else {
            log_lines.push("  removed HideDesktopApps.vbs".to_string());
        }
    }

    // write a log file so we know what got migrated
    let log_path = dir.join("migration.log");
    let _ = std::fs::write(&log_path, log_lines.join("\n") + "\n");

    Ok(config)
}

pub fn load_config() -> Result<AppConfig> {
    let path = config_path()?;

    if !path.exists() {
        // check if there's an old config.ini to migrate from
        let dir = config_dir()?;
        let ini_path = dir.join("config.ini");
        if ini_path.exists() {
            eprintln!("Migrating config.ini to config.toml");
            match migrate_ini(&ini_path, &dir) {
                Ok(cfg) => {
                    save_config(&cfg)?;
                    // rename it so we don't try to migrate again
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

    let config: AppConfig = toml::from_str(&content).with_context(|| "Parsing config.toml")?;

    Ok(config)
}

pub fn save_config(config: &AppConfig) -> Result<()> {
    let path = config_path()?;
    let content = toml::to_string_pretty(config).context("Serializing config")?;
    std::fs::write(&path, content)
        .with_context(|| format!("Writing config to {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_roundtrips_through_toml() {
        let cfg = AppConfig::default();
        let text = toml::to_string_pretty(&cfg).expect("serialize");
        let back: AppConfig = toml::from_str(&text).expect("deserialize");
        assert_eq!(back.hotkeys.icons, cfg.hotkeys.icons);
        assert_eq!(back.updater.channel, cfg.updater.channel);
        assert_eq!(back.profiles.len(), cfg.profiles.len());
        assert_eq!(back.window_filter.exclude_processes, cfg.window_filter.exclude_processes);
    }

    #[test]
    fn toml_without_profiles_section_defaults_to_empty() {
        // `profiles` is the only #[serde(default)] field; omitting it must not error.
        let text = "\
[hotkeys]
icons = \"ctrl+alt+h\"
taskbar = \"ctrl+alt+t\"
windows = \"ctrl+alt+w\"

[startup]
enabled = true
delay_s = 30

[defaults]
profile = \"\"

[updater]
enabled = true
channel = \"stable\"
check_interval_h = 24
last_checked = \"\"

[notifications]
enabled = true
on_update = true
on_hotkey_fail = true
on_profile_switch = false

[discord]
enabled = true

[window_filter]
exclude_processes = []
";
        let back: AppConfig = toml::from_str(text).expect("deserialize without profiles");
        assert!(back.profiles.is_empty());
    }

    #[test]
    fn parse_ini_reads_hotkeys_and_flags() {
        let ini = "\
[Settings]
hotkey_icons = ctrl+alt+x
run_on_startup = false
startup_delay = 12
auto_update = 0
[Hotkeys]
taskbar = ctrl+alt+b
";
        let (cfg, icons_hidden) = parse_ini(ini);
        assert_eq!(cfg.hotkeys.icons, "ctrl+alt+x");
        assert_eq!(cfg.hotkeys.taskbar, "ctrl+alt+b");
        assert!(!cfg.startup.enabled);
        assert_eq!(cfg.startup.delay_s, 12);
        assert!(!cfg.updater.enabled);
        assert!(!icons_hidden);
    }

    #[test]
    fn parse_ini_icons_hidden_sets_flag() {
        let (_, icons_hidden) = parse_ini("icons_hidden = true\n");
        assert!(icons_hidden);
    }

    #[test]
    fn parse_ini_ignores_comments_and_blanks() {
        let (cfg, _) = parse_ini("# comment\n; comment\n\n   \n");
        // untouched -> defaults
        assert_eq!(cfg.hotkeys.icons, HotkeysConfig::default().icons);
    }
}
