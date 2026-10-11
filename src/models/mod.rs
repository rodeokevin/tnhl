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
// Variant names are the NHL's own uppercase team codes
#[allow(clippy::upper_case_acronyms)]
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
    // and scoring data. Every code must be classified in `is_franchise`.
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

    /// Unknown
    #[serde(untagged)]
    #[strum(default)]
    Unknown(UnknownTeamCode),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownTeamCode {
    bytes: [u8; Self::MAX_LEN],
    len: u8,
}

impl UnknownTeamCode {
    const MAX_LEN: usize = 8;

    pub fn as_str(&self) -> &str {
        // Only ever filled from a `&str` cut at a char boundary
        std::str::from_utf8(&self.bytes[..self.len as usize]).unwrap_or_default()
    }
}

impl From<&str> for UnknownTeamCode {
    fn from(code: &str) -> Self {
        let mut end = code.len().min(Self::MAX_LEN);
        while !code.is_char_boundary(end) {
            end -= 1;
        }
        let mut bytes = [0; Self::MAX_LEN];
        bytes[..end].copy_from_slice(&code.as_bytes()[..end]);
        Self {
            bytes,
            len: end as u8,
        }
    }
}

impl std::fmt::Display for UnknownTeamCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for UnknownTeamCode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let code = String::deserialize(deserializer)?;
        Ok(Self::from(code.as_str()))
    }
}

impl TeamAbbrev {
    /// Whether this is an NHL franchise (current or defunct), as opposed to an
    /// All-Star, national, or exhibition team from a special event, or a code
    /// the app doesn't recognize.
    ///
    /// Deliberately exhaustive with no wildcard, so adding a code fails to
    /// compile until it's classified here.
    pub fn is_franchise(&self) -> bool {
        use TeamAbbrev::*;
        match self {
            AFM | ANA | ARI | ATL | BOS | BRK | BUF | CAR | CAT | CBJ | CBN | CGS | CGY | CHI
            | CLE | CLR | COL | DAL | DCG | DET | DFL | EDE | EDM | FLA | HAM | HFD | KCS | LAK
            | MIN | MMR | MNS | MTL | MWN | NJD | NSH | NYA | NYI | NYR | OAK | OTT | PHI | PHX
            | PIR | PIT | QBD | QUA | QUE | SEA | SEN | SLE | SJS | SMT | STL | TAN | TBD | TBL
            | TOR | TSP | UTA | VAN | VGK | VIC | VMI | VMA | WIN | WPG | WSH => true,

            FirstAllStars | SecondAllStars | ALL | ASB | ASE | ASR | ASW | AWA | BLK | BLU
            | CEN | CMP | CWA | HGS | KLS | KNG | MAT | MCD | MKN | NorthAmericaAllStars | PAC
            | RED | WHT | WLS | WRL | YSE | YSW | AUT | BLR | CAN | CZE | DEN | DEU | EUR | FIN
            | FRA | FRG | GER | ITA | JPN | KAZ | LAT | NAT | NOR | RUS | SLO | SUI | SVK | SWE
            | TCH | UKR | URS | USA | AIK | BFT | BRY | CSK | DAV | DDF | DIN | DJU | EIS | EVZ
            | FAR | FEL | GOT | GRA | HBF | HEL | IFK | ILV | KAR | KHI | KLA | KLH | LAU | LIB
            | LIN | LNY | MAL | MAN | MET | MOD | MOS | MUN | PET | PHT | RBS | RIG | SCB | SKA
            | SLV | SOK | SPA | SPR | SWI | TAP | TXS | VFF | ZUR => false,

            Unknown(_) => false,
        }
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
    fn unknown_codes_keep_their_text() {
        let a = parse("QQQ");
        let b = parse("XYZ");
        assert_eq!(a.to_string(), "QQQ");
        assert_ne!(a, b, "different unknown teams must not compare equal");
        assert_eq!(a, parse("QQQ"));
        assert!(!a.is_franchise());
    }

    #[test]
    fn known_codes_still_parse_to_their_variant() {
        assert_eq!(parse("MTL"), TeamAbbrev::MTL);
        assert!(matches!(parse("Unknown"), TeamAbbrev::Unknown(_)));
    }

    #[test]
    fn long_unknown_codes_are_truncated_safely() {
        assert_eq!(parse("ABCDEFGHIJK").to_string(), "ABCDEFGH");
        // Never splits a multi-byte character
        assert_eq!(parse("ÉÉÉÉÉ").to_string(), "ÉÉÉÉ");
    }

    #[test]
    fn unknown_codes_are_rejected_by_from_str_callers() {
        // FromStr falls back to Unknown instead of failing; callers that need a
        // real team check is_franchise
        let team: TeamAbbrev = "ZZZ".parse().unwrap();
        assert!(!team.is_franchise());
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
