use crate::models::TeamAbbrev;
use crate::state::app_settings::AppSettings;
use anyhow::Context;
use chrono::{TimeZone, Utc};
use chrono_tz::{OffsetName, Tz};
use directories::ProjectDirs;
use log::{LevelFilter, error};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Off,
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ConfigFile {
    /// Your favorite team's 3-letter abbreviation (e.g. "MTL", "TOR").
    /// When set, it becomes the default team on the Team Stats page and is
    /// highlighted in the standings and in today's game matchups.
    /// Case-insensitive. See the `TeamAbbrev` enum for all valid codes.
    pub favorite_team: Option<String>,

    /// Timezone to display game start times in. Common options are:
    /// * "US/Pacific"
    /// * "US/Mountain"
    /// * "US/Central"
    /// * "US/Eastern"
    ///
    /// For the full list see https://en.wikipedia.org/wiki/List_of_tz_database_time_zones.
    pub timezone: Option<Tz>,

    /// Optional log level to use. If not present, the default is `Error`.
    /// Set the level using a lowercase string, e.g. "error".
    pub log_level: Option<LogLevel>,
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            favorite_team: None,
            timezone: Some(ConfigFile::DEFAULT_TIMEZONE),
            log_level: None,
        }
    }
}

#[allow(clippy::from_over_into)]
impl Into<AppSettings> for ConfigFile {
    fn into(self) -> AppSettings {
        AppSettings {
            favorite_team: self.validate_favorite_team(),
            timezone: self.validate_timezone(),
            timezone_abbreviation: self.get_timezone_abbreviation(),
            log_level: self.validate_log_level(),
        }
    }
}

static CONFIG_FILE_LOCATION: OnceLock<Option<PathBuf>> = OnceLock::new();

impl ConfigFile {
    const DEFAULT_TIMEZONE: Tz = chrono_tz::America::Montreal;
    const CONFIG_FILE_NAME: &'static str = "tnhl.toml";
    const LOG_FILE_NAME: &'static str = "tnhl.log";

    pub fn load_from_file() -> anyhow::Result<ConfigFile> {
        if let Some(path) = Self::get_config_location() {
            if !path.exists() {
                Self::generate_config_file(&path)?;
            }
            Self::load_config_file(&path)
        } else {
            anyhow::bail!("could not find config file");
        }
    }

    /// Parse the configured favorite team abbreviation into a `TeamAbbrev`.
    /// Invalid or missing values yield `None` (logged, non-fatal).
    fn validate_favorite_team(&self) -> Option<TeamAbbrev> {
        let favorite = self.favorite_team.as_ref()?;
        match TeamAbbrev::from_str(favorite.trim()) {
            // Event teams (All-Stars, national teams) aren't valid favorites
            Ok(team) if team.is_franchise() => Some(team),
            _ => {
                error!("invalid favorite_team in config: {favorite:?}");
                None
            }
        }
    }

    fn validate_timezone(&self) -> Tz {
        self.timezone.unwrap_or(Self::DEFAULT_TIMEZONE)
    }

    fn validate_log_level(&self) -> Option<LevelFilter> {
        self.log_level.map(|level| match level {
            LogLevel::Off => LevelFilter::Off,
            LogLevel::Trace => LevelFilter::Trace,
            LogLevel::Debug => LevelFilter::Debug,
            LogLevel::Info => LevelFilter::Info,
            LogLevel::Warn => LevelFilter::Warn,
            LogLevel::Error => LevelFilter::Error,
        })
    }

    /// Get the abbreviated name of the configured timezone, (e.g. "PST" or "PDT")
    fn get_timezone_abbreviation(&self) -> String {
        let tz = self.timezone.unwrap_or(Self::DEFAULT_TIMEZONE);
        let now = Utc::now().with_timezone(&tz).naive_utc();
        let offset = tz.offset_from_utc_datetime(&now);
        offset.abbreviation().unwrap_or("~~").to_string()
    }

    /// Generate the path of the config file for the current operating system:
    /// * Linux:   /home/alice/.config/tnhl/tnhl.toml
    /// * Windows: C:\Users\Alice\AppData\Roaming\tnhl\tnhl.toml
    /// * macOS:   /Users/Alice/Library/Application Support/tnhl/tnhl.toml
    pub fn get_config_location() -> Option<PathBuf> {
        CONFIG_FILE_LOCATION
            .get_or_init(|| {
                if let Some(proj_dirs) = ProjectDirs::from("", "", "tnhl") {
                    let dir = proj_dirs.config_dir();
                    if !dir.exists()
                        && let Err(err) = std::fs::create_dir_all(dir)
                    {
                        error!("could not create config dir: {err:?}");
                    }
                    let config_file = dir.join(Self::CONFIG_FILE_NAME);
                    Some(config_file)
                } else {
                    error!("could not get valid home directory for config file");
                    None
                }
            })
            .clone()
    }

    /// Generate the path of the log file for the current operating system:
    /// * Linux:   /home/alice/.local/share/tnhl/tnhl.log
    /// * Windows: C:\Users\Alice\AppData\Roaming\tnhl\data\tnhl.log
    /// * macOS:   /Users/Alice/Library/Application Support/tnhl/tnhl.log
    pub fn get_log_location() -> Option<PathBuf> {
        let proj_dirs = ProjectDirs::from("", "", "tnhl")?;
        let dir = proj_dirs.data_dir();
        if !dir.exists()
            && let Err(err) = std::fs::create_dir_all(dir)
        {
            error!("could not create data dir for log file: {err:?}");
            return None;
        }
        Some(dir.join(Self::LOG_FILE_NAME))
    }

    fn generate_config_file(path: &PathBuf) -> anyhow::Result<()> {
        let contents =
            toml::to_string(&ConfigFile::default()).context("could not serialize config")?;
        let contents = format!(
            "# See https://github.com/rodeokevin/TNHL#configuration for options\n{contents}"
        );
        std::fs::write(path, contents).context("could not write config file")
    }

    fn load_config_file(path: &PathBuf) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path).context("could not read config file")?;
        toml::from_str(&contents).context("could not deserialize config file")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with_favorite(favorite: Option<&str>) -> ConfigFile {
        ConfigFile {
            favorite_team: favorite.map(|s| s.to_string()),
            ..ConfigFile::default()
        }
    }

    #[test]
    fn valid_favorite_team_parses() {
        let cfg = config_with_favorite(Some("TOR"));
        assert_eq!(cfg.validate_favorite_team(), Some(TeamAbbrev::TOR));
    }

    #[test]
    fn favorite_team_is_case_insensitive_and_trimmed() {
        assert_eq!(
            config_with_favorite(Some("mtl")).validate_favorite_team(),
            Some(TeamAbbrev::MTL)
        );
        assert_eq!(
            config_with_favorite(Some("  VgK ")).validate_favorite_team(),
            Some(TeamAbbrev::VGK)
        );
    }

    #[test]
    fn event_team_is_not_a_valid_favorite() {
        assert_eq!(
            config_with_favorite(Some("CAN")).validate_favorite_team(),
            None
        );
    }

    #[test]
    fn invalid_favorite_team_is_none() {
        assert_eq!(
            config_with_favorite(Some("ZZZ")).validate_favorite_team(),
            None
        );
        assert_eq!(
            config_with_favorite(Some("")).validate_favorite_team(),
            None
        );
    }

    #[test]
    fn missing_favorite_team_is_none() {
        assert_eq!(config_with_favorite(None).validate_favorite_team(), None);
    }
}
