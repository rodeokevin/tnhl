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

    /// Problems found while loading the file. The config is read before
    /// logging starts, so these are reported once the logger is up.
    #[serde(skip)]
    pub warnings: Vec<String>,
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            favorite_team: None,
            timezone: Some(ConfigFile::DEFAULT_TIMEZONE),
            log_level: None,
            warnings: Vec::new(),
        }
    }
}

#[allow(clippy::from_over_into)]
impl Into<AppSettings> for ConfigFile {
    fn into(mut self) -> AppSettings {
        let favorite_team = self.validate_favorite_team();
        if favorite_team.is_none()
            && let Some(favorite) = &self.favorite_team
        {
            self.warnings.push(format!(
                "favorite_team {favorite:?} is not an NHL team code; ignoring it"
            ));
        }
        AppSettings {
            favorite_team,
            timezone: self.validate_timezone(),
            timezone_abbreviation: self.get_timezone_abbreviation(),
            log_level: self.validate_log_level(),
            config_warnings: self.warnings,
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
    /// Invalid or missing values yield `None`.
    fn validate_favorite_team(&self) -> Option<TeamAbbrev> {
        let favorite = self.favorite_team.as_ref()?;
        match TeamAbbrev::from_str(favorite.trim()) {
            // Event teams (All-Stars, national teams) aren't valid favorites
            Ok(team) if team.is_franchise() => Some(team),
            _ => None,
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
            "# See https://github.com/rodeokevin/tnhl#configuration for options\n{contents}"
        );
        std::fs::write(path, contents).context("could not write config file")
    }

    fn load_config_file(path: &PathBuf) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path).context("could not read config file")?;
        Self::parse(&contents)
    }

    /// Parse each setting on its own, so one bad value only drops that setting
    /// (with a warning) instead of discarding the whole file. Only invalid TOML
    /// syntax fails the whole parse.
    fn parse(contents: &str) -> anyhow::Result<Self> {
        let mut table: toml::Table =
            toml::from_str(contents).context("config file is not valid TOML")?;
        let mut warnings = Vec::new();

        let favorite_team = take_setting(&mut table, "favorite_team", &mut warnings);
        let timezone =
            take_setting(&mut table, "timezone", &mut warnings).or(Some(Self::DEFAULT_TIMEZONE));
        let log_level = take_setting(&mut table, "log_level", &mut warnings);

        for key in table.keys() {
            warnings.push(format!("unknown setting {key:?}; ignoring it"));
        }

        Ok(Self {
            favorite_team,
            timezone,
            log_level,
            warnings,
        })
    }
}

/// Remove `key` from the table and parse it, recording a warning and
/// returning `None` if the value is invalid.
fn take_setting<T: serde::de::DeserializeOwned>(
    table: &mut toml::Table,
    key: &str,
    warnings: &mut Vec<String>,
) -> Option<T> {
    let value = table.remove(key)?;
    let shown = value.to_string();
    match value.try_into() {
        Ok(parsed) => Some(parsed),
        Err(err) => {
            let reason = err.message().trim().to_string();
            warnings.push(format!("invalid {key} {shown}: {reason}; ignoring it"));
            None
        }
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
    fn bad_setting_only_drops_that_setting() {
        let cfg = ConfigFile::parse(
            "timezone = \"US/Easter\"\nfavorite_team = \"MTL\"\nlog_level = \"debug\"\n",
        )
        .unwrap();
        assert_eq!(cfg.timezone, Some(ConfigFile::DEFAULT_TIMEZONE));
        assert_eq!(cfg.favorite_team.as_deref(), Some("MTL"));
        assert!(matches!(cfg.log_level, Some(LogLevel::Debug)));
        assert_eq!(cfg.warnings.len(), 1);
        assert!(cfg.warnings[0].contains("timezone"));
    }

    #[test]
    fn valid_config_has_no_warnings() {
        let cfg = ConfigFile::parse(
            "timezone = \"US/Eastern\"\nfavorite_team = \"MTL\"\nlog_level = \"info\"\n",
        )
        .unwrap();
        assert_eq!(cfg.timezone, Some(chrono_tz::US::Eastern));
        assert!(cfg.warnings.is_empty());
    }

    #[test]
    fn unknown_and_invalid_settings_warn() {
        let settings: AppSettings = ConfigFile::parse(
            "favourite_team = \"MTL\"\nlog_level = \"loud\"\nfavorite_team = \"ZZZ\"\n",
        )
        .unwrap()
        .into();
        assert_eq!(settings.favorite_team, None);
        assert_eq!(settings.log_level, None);
        assert_eq!(settings.config_warnings.len(), 3);
    }

    #[test]
    fn invalid_toml_fails_whole_file() {
        assert!(ConfigFile::parse("timezone = \"US/Eastern").is_err());
    }

    #[test]
    fn missing_favorite_team_is_none() {
        assert_eq!(config_with_favorite(None).validate_favorite_team(), None);
    }
}
