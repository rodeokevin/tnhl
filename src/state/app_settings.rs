use crate::config::ConfigFile;
use crate::models::TeamAbbrev;
use chrono_tz::Tz;

#[derive(Debug, Default, Clone)]
pub struct AppSettings {
    pub favorite_team: Option<TeamAbbrev>,
    pub timezone: Tz,
    pub timezone_abbreviation: String,
    pub log_level: Option<log::LevelFilter>,
    /// Problems found in the config file, reported once logging is set up
    pub config_warnings: Vec<String>,
}

impl AppSettings {
    /// If the config file can't be loaded, fall back to defaults and record a
    /// warning rather than blocking the app from starting
    pub fn load_from_file() -> Self {
        ConfigFile::load_from_file()
            .unwrap_or_else(|err| ConfigFile {
                warnings: vec![format!(
                    "could not load config file ({err:#}); using defaults"
                )],
                ..ConfigFile::default()
            })
            .into()
    }
}
