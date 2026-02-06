use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub wezterm: WeztermConfig,
    pub layout: LayoutConfig,
    pub tui: TuiConfig,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct WeztermConfig {
    pub binary: String,
    pub claude_command: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct LayoutConfig {
    pub watcher_width: u32,
    pub shell_height: u32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct TuiConfig {
    pub tick_interval_secs: u64,
}

impl Default for WeztermConfig {
    fn default() -> Self {
        Self {
            binary: "wezterm".to_string(),
            claude_command: "claude".to_string(),
        }
    }
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            watcher_width: 20,
            shell_height: 30,
        }
    }
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            tick_interval_secs: 3,
        }
    }
}

fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|c| c.join("ccm").join("config.toml"))
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let path = match config_path() {
            Some(p) => p,
            None => return Ok(Self::default()),
        };

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "failed to read config file {}: {e}",
                    path.display()
                ));
            }
        };

        let config: Config = toml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("failed to parse config file {}: {e}", path.display()))?;

        config.validate()?;

        Ok(config)
    }

    fn validate(&self) -> anyhow::Result<()> {
        if self.wezterm.binary.is_empty() {
            anyhow::bail!("config error: wezterm.binary must not be empty");
        }
        if self.tui.tick_interval_secs == 0 {
            anyhow::bail!("config error: tui.tick_interval_secs must be >= 1");
        }
        if self.layout.watcher_width == 0 || self.layout.watcher_width >= 100 {
            anyhow::bail!("config error: layout.watcher_width must be between 1 and 99");
        }
        if self.layout.shell_height == 0 || self.layout.shell_height >= 100 {
            anyhow::bail!("config error: layout.shell_height must be between 1 and 99");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_expected_values() {
        let config = Config::default();
        assert_eq!(config.wezterm.binary, "wezterm");
        assert_eq!(config.wezterm.claude_command, "claude");
        assert_eq!(config.layout.watcher_width, 20);
        assert_eq!(config.layout.shell_height, 30);
        assert_eq!(config.tui.tick_interval_secs, 3);
    }

    #[test]
    fn parse_full_toml() {
        let toml_str = r#"
[wezterm]
binary = "/usr/local/bin/wezterm"
claude_command = "claude --model opus"

[layout]
watcher_width = 25
shell_height = 40

[tui]
tick_interval_secs = 5
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.wezterm.binary, "/usr/local/bin/wezterm");
        assert_eq!(config.wezterm.claude_command, "claude --model opus");
        assert_eq!(config.layout.watcher_width, 25);
        assert_eq!(config.layout.shell_height, 40);
        assert_eq!(config.tui.tick_interval_secs, 5);
    }

    #[test]
    fn parse_partial_toml_fills_defaults() {
        let toml_str = r#"
[tui]
tick_interval_secs = 10
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.wezterm.binary, "wezterm");
        assert_eq!(config.wezterm.claude_command, "claude");
        assert_eq!(config.layout.watcher_width, 20);
        assert_eq!(config.layout.shell_height, 30);
        assert_eq!(config.tui.tick_interval_secs, 10);
    }

    #[test]
    fn parse_empty_toml_returns_defaults() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config.wezterm.binary, "wezterm");
        assert_eq!(config.tui.tick_interval_secs, 3);
    }

    #[test]
    fn malformed_toml_returns_error() {
        let result: Result<Config, _> = toml::from_str("[invalid");
        assert!(result.is_err());
    }

    #[test]
    fn validate_rejects_empty_binary() {
        let mut config = Config::default();
        config.wezterm.binary = String::new();
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_zero_tick_interval() {
        let mut config = Config::default();
        config.tui.tick_interval_secs = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_zero_watcher_width() {
        let mut config = Config::default();
        config.layout.watcher_width = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_100_watcher_width() {
        let mut config = Config::default();
        config.layout.watcher_width = 100;
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_zero_shell_height() {
        let mut config = Config::default();
        config.layout.shell_height = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_100_shell_height() {
        let mut config = Config::default();
        config.layout.shell_height = 100;
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_accepts_valid_config() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }
}
