//! Data models for NHL API responses.
//!
//! These structs mirror the shape of the public NHL Web API JSON. Many fields
//! are parsed for completeness/documentation even when the UI does not yet read
//! them, so `dead_code` is allowed across this module tree.
#![allow(dead_code)]

use serde::Deserialize;
use strum_macros::{Display, EnumString};

pub mod games;
pub mod playoffs;
pub mod standings;
pub mod team_stats;

#[derive(Deserialize, Debug)]
pub struct TeamName {
    pub default: String,
}

#[derive(Deserialize, Debug)]
pub struct PlaceName {
    pub default: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Display, EnumString, Deserialize)]
#[strum(ascii_case_insensitive)]
pub enum TeamAbbrev {
    AFM, // Atlanta Flames (defunct)
    ANA, // Anaheim Ducks
    ARI, // Arizona Coyotes (defunct)
    ATL, // Atlanta Thrasher (defunct)
    BOS,
    BRK, // Brooklyn Americans (defunct)
    BUF,
    CAR,
    CAT, // Calgary Tigers (WCHL, defunct)
    CBJ,
    CBN, // Cleveland Barons (defunct)
    CGS, // Bay Area Seals/California Golden Seals (Defunct)
    CGY,
    CHI,
    CLE, // Cleveland Barons (defunct)
    CLR, // Colorado Rockies (defunct)
    COL,
    DAL,
    DCG, // Detroit Cougars (defunct)
    DET,
    DFL, // Detroit Falcons (defunct)
    EDE, // Edmonton Eskimos (WCHL, defunct)
    EDM,
    FLA,
    HAM, // Hamilton Tigers (defunct)
    HFD,
    KCS, // Kansas City Scouts (defunct)
    LAK,
    MIN,
    MMR, // Montreal Marrons (defunct)
    MNS, // Minnesota North Stars (defunct)
    #[default]
    MTL,
    MWN, // Montreal Wanderers (defunct)
    NJD,
    NSH,
    NYA, // New York Americans (defunct)
    NYI,
    NYR,
    OAK, // California/Oakland Seals (defunct)
    OTT,
    PHI,
    PHX, // Phoenix Coyotes (defunct)
    PIR, // Pittsburgh Pirates (defunct)
    PIT,
    QBD, // Quebec Bulldogs (defunct)
    QUA, // Philadelphia Quakers(defunct)
    QUE, // Quebec Nordiques (defunct)
    SEA,
    SEN, // Ottawa Senators (original)
    SLE, // St. Louis Eagles (defunct)
    SJS,
    SMT,
    STL,
    TAN, // Toronto Hockey Club/Toronto Arenas (defunct)
    TBD, // To be determined
    TBL,
    TOR,
    TSP, // Toronto St. Patricks (defunct)
    UTA,
    VAN,
    VGK,
    VIC, // Victoria Cougars (WCHL/WHL, defunct)
    VMI, // Vancouver Millionaires (defunct)
    VMA, // Montreal Maroons (defunct)
    WIN, // Winnipeg Jets (original)
    WPG,
    WSH,
    // ---------------------------------------------------------------------
    // Non-franchise teams from special events. These only appear on game
    // and scoring data. Keep them after `WSH`: `is_franchise` relies on it.
    // ---------------------------------------------------------------------
    // All-Star, Young Stars, and skills-event teams
    #[serde(rename = "1ST")]
    #[strum(serialize = "1ST")]
    FirstAllStars, // First All-Star Team
    #[serde(rename = "2ND")]
    #[strum(serialize = "2ND")]
    SecondAllStars, // Second All-Star Team
    ALL, // NHL All-Stars (vs. the Cup champion)
    ASB, // Team Lidstrom
    ASE, // All-Stars East
    ASR, // Team Staal
    ASW, // All-Stars West
    AWA, // American All-Stars
    BLK, // Team Foligno
    BLU, // Team Chara
    CEN, // Central Division All-Stars
    CMP, // Campbell Conference All-Stars
    CWA, // Canadian All-Stars
    HGS, // Team Hughes
    KLS, // Team Kloss
    KNG, // Team King
    MAT, // Team Matthews
    MCD, // Team McDavid
    MKN, // Team MacKinnon
    #[serde(rename = "N.A")]
    #[strum(serialize = "N.A")]
    NorthAmericaAllStars, // North America All-Stars
    PAC, // Pacific Division All-Stars
    RED, // Team Alfredsson
    WHT, // Team Toews
    WLS, // Wales Conference All-Stars
    WRL, // World All-Stars
    YSE, // Young Stars East
    YSW, // Young Stars West
    // National and international teams (Canada Cup, World Cup, Olympics, 4 Nations, ...)
    AUT, // Austria
    BLR, // Belarus
    CAN, // Canada
    CZE, // Czech Republic/Czechia
    DEN, // Denmark
    DEU, // Germany (World Cup)
    EUR, // Europe
    FIN, // Finland
    FRA, // France
    FRG, // West Germany
    GER, // Germany (Olympics)
    ITA, // Italy
    JPN, // Japan
    KAZ, // Kazakhstan
    LAT, // Latvia
    NAT, // North America
    NOR, // Norway
    RUS, // Russia
    SLO, // Slovenia
    SUI, // Switzerland
    SVK, // Slovakia
    SWE, // Sweden
    TCH, // Czechoslovakia
    UKR, // Ukraine
    URS, // Soviet Union
    USA, // USA
    // European and other clubs from preseason exhibitions
    AIK, // AIK IF
    BFT, // Belfast Giants Select
    BRY, // Brynas If Gavle
    CSK, // CSKA Moscow
    DAV, // HC Davos
    DDF, // Düsseldorfer EG
    DIN, // Dinamo Riga
    DJU, // Djurgardens IF Stockholm
    EIS, // Eisbären Berlin
    EVZ, // EV Zug
    FAR, // Farjestads Bk Karlstad
    FEL, // Feldkirch
    GOT, // Frolunda HC
    GRA, // Graz 99ers
    HBF, // Hamburg Freezers
    HEL, // Jokerit Helsinki
    IFK, // HIFK Helsinki
    ILV, // Ilves Tampere
    KAR, // Karpat Oulu
    KHI, // Khimik Voskresensk
    KLA, // Klagenfurt
    KLH, // Kölner Haie
    LAU, // Lausanne
    LIB, // Liberec
    LIN, // Linkopings HC
    LNY, // Torpedo Yaroslavl
    MAL, // Malmö Redhawks
    MAN, // Adler Mannheim
    MET, // Metallurg Magnitogorsk (exhibition) / Metropolitan Division All-Stars
    MOD, // Modo Ornskoldsvik
    MOS, // Dynamo Moscow
    MUN, // EHC Red Bull München
    PET, // Ska St. Petersburg
    PHT, // Adirondack Phantoms
    RBS, // EC Red Bull Salzburg
    RIG, // Dynamo Riga
    SCB, // SC Bern
    SKA, // Ska Leningrad
    SLV, // HC Slovan
    SOK, // Sokol Kiev
    SPA, // Spartak
    SPR, // HC Sparta Praha
    SWI, // Soviet Wings
    TAP, // Tappara Tampere
    TXS, // Texas Stars
    VFF, // Vastra Frolunda
    ZUR, // ZSC Lions Zurich
}

impl TeamAbbrev {
    /// The first non-franchise (special event) variant.
    const FIRST_EVENT_TEAM: TeamAbbrev = TeamAbbrev::FirstAllStars;

    /// Whether this is an NHL franchise (current or defunct), as opposed to an
    /// All-Star, national, or exhibition team from a special event.
    pub fn is_franchise(&self) -> bool {
        (*self as u16) < (Self::FIRST_EVENT_TEAM as u16)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TeamAbbrevWrapper {
    pub default: TeamAbbrev,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(code: &str) -> TeamAbbrev {
        serde_json::from_str(&format!("\"{code}\"")).unwrap()
    }

    #[test]
    fn event_team_codes_parse() {
        assert_eq!(parse("FIN"), TeamAbbrev::FIN);
        assert_eq!(parse("HGS"), TeamAbbrev::HGS);
        assert_eq!(parse("1ST"), TeamAbbrev::FirstAllStars);
        assert_eq!(parse("N.A"), TeamAbbrev::NorthAmericaAllStars);
        assert_eq!(TeamAbbrev::NorthAmericaAllStars.to_string(), "N.A");
    }

    #[test]
    fn only_franchises_are_franchises() {
        assert!(TeamAbbrev::AFM.is_franchise());
        assert!(TeamAbbrev::MTL.is_franchise());
        assert!(TeamAbbrev::WSH.is_franchise());
        assert!(!TeamAbbrev::FirstAllStars.is_franchise());
        assert!(!TeamAbbrev::CAN.is_franchise());
        assert!(!TeamAbbrev::ZUR.is_franchise());
    }
}
