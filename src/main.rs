mod app;
mod banner;
mod config;
mod input;
mod models;
mod sources;
mod state;
mod ui;

use crate::{
    app::App,
    sources::{
        AppEvent, Source,
        games::{
            boxscore::{BoxscoreCommand, BoxscoreSource},
            game_story::{GameStoryCommand, GameStorySource},
            games::{GamesCommand, GamesSource},
            play_by_play::{PlaysCommand, PlaysSource},
            total_goals::{TotalGoalsCommand, TotalGoalsSource},
        },
        playoffs::{
            bracket::{BracketCommand, BracketSource},
            bracket_series::{BracketSeriesCommand, BracketSeriesSource},
            series::{SeriesCommand, SeriesSource},
        },
        season::SeasonSource,
        standings::{StandingsCommand, StandingsSource},
        teams_stats::{TeamStatsCommand, TeamStatsSource},
        today::{TodaySource, fetch_today},
    },
};

use simplelog::*;
use std::fs::File;

use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
    crossterm::{
        event::{DisableMouseCapture, EnableMouseCapture, Event, EventStream},
        execute,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
};
use std::{error::Error, io};
use tokio::sync::mpsc::Receiver;
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

use futures::StreamExt;

/// Resolve the current game day from the NHL endpoint and fallback to system date
async fn resolve_today_from_api(app: &mut App, client: &reqwest::Client) {
    // Short so a slow network doesn't hold up startup
    const STARTUP_TIMEOUT: Duration = Duration::from_secs(2);

    match fetch_today(client, STARTUP_TIMEOUT).await {
        Some(date) => {
            log::debug!("Resolved current game day from API: {}", date);
            app.state.date_state.date = date;
            app.state.date_state.today = date;
        }
        None => log::warn!(
            "Falling back to local date {} for current game day",
            app.state.date_state.date
        ),
    }
}

/// Upper bound on any single NHL API request, so a stalled connection can't
/// hold one of the shared request slots forever.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Restore the terminal before printing a panic, so a crash doesn't leave the
/// shell in raw mode on the alternate screen.
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stderr(), LeaveAlternateScreen, DisableMouseCapture);
        default_hook(info);
    }));
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging before entering the alternate screen so any warning
    // printed to stderr lands on the normal terminal rather than corrupting the
    // TUI. File logging is best-effort: if we can't resolve a data dir or
    // create the log file, warn and continue without it rather than preventing
    // the app from starting.
    let settings = crate::state::app_settings::AppSettings::load_from_file();
    let log_level = settings.log_level.unwrap_or(LevelFilter::Error);
    match config::ConfigFile::get_log_location() {
        Some(path) => match File::create(&path) {
            Ok(log_file) => {
                if let Err(err) = WriteLogger::init(log_level, Config::default(), log_file) {
                    eprintln!("could not initialize logger: {err}");
                }
            }
            Err(err) => eprintln!("could not create log file at {path:?}: {err}"),
        },
        None => eprintln!("could not resolve a log file location; file logging disabled"),
    }

    // A single shared HTTP client is reused by every source. Built before the
    // terminal is set up so a failure here doesn't leave it in raw mode.
    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()?;

    install_panic_hook();

    // setup terminal
    enable_raw_mode()?;
    let mut stderr = io::stderr();
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let (games_cmd_tx, games_cmd_rx) = tokio::sync::mpsc::channel(8);
    let (standings_cmd_tx, standings_cmd_rx) = tokio::sync::mpsc::channel(8);
    let (boxscore_cmd_tx, boxscore_cmd_rx) = tokio::sync::mpsc::channel(8);
    let (plays_cmd_tx, plays_cmd_rx) = tokio::sync::mpsc::channel(8);
    let (game_story_tx, game_story_rx) = tokio::sync::mpsc::channel(8);
    let (team_stats_tx, team_stats_rx) = tokio::sync::mpsc::channel(8);
    let (bracket_tx, bracket_rx) = tokio::sync::mpsc::channel(8);
    let (series_tx, series_rx) = tokio::sync::mpsc::channel(8);
    let (total_goals_tx, total_goals_rx) = tokio::sync::mpsc::channel(8);
    let (bracket_series_tx, bracket_series_rx) = tokio::sync::mpsc::channel(8);

    // Date is configured in here
    let mut app = App::new(
        settings,
        games_cmd_tx.clone(),
        standings_cmd_tx.clone(),
        boxscore_cmd_tx.clone(),
        plays_cmd_tx.clone(),
        game_story_tx.clone(),
        team_stats_tx.clone(),
        bracket_tx.clone(),
        series_tx.clone(),
        total_goals_tx.clone(),
        bracket_series_tx.clone(),
    );
    let cancel = CancellationToken::new();
    // Only the starting tab's sources poll; the rest wait until visited
    app.state.sync_active_sources();

    resolve_today_from_api(&mut app, &client).await;

    let _ = run_app(
        &mut terminal,
        &mut app,
        client,
        cancel.clone(),
        games_cmd_rx,
        standings_cmd_rx,
        boxscore_cmd_rx,
        plays_cmd_rx,
        game_story_rx,
        team_stats_rx,
        bracket_rx,
        series_rx,
        total_goals_rx,
        bracket_series_rx,
    )
    .await;

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

async fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    client: reqwest::Client,
    cancel: CancellationToken,
    games_rx: Receiver<GamesCommand>,
    standings_rx: Receiver<StandingsCommand>,
    boxscore_rx: Receiver<BoxscoreCommand>,
    plays_rx: Receiver<PlaysCommand>,
    game_story_rx: Receiver<GameStoryCommand>,
    team_stats_rx: Receiver<TeamStatsCommand>,
    bracket_rx: Receiver<BracketCommand>,
    series_rx: Receiver<SeriesCommand>,
    total_goals_rx: Receiver<TotalGoalsCommand>,
    bracket_series_rx: Receiver<BracketSeriesCommand>,
) -> io::Result<()>
where
    io::Error: From<B::Error>,
{
    let (tx, mut rx) = tokio::sync::mpsc::channel::<AppEvent>(32);

    // Spawn standings source
    let standings_source = Box::new(StandingsSource::new(
        client.clone(),
        standings_rx,
        app.state.date_state.date.to_string(),
    ));
    let standings_tx = tx.clone();
    let standings_cancel = cancel.clone();
    tokio::spawn(async move {
        standings_source.run(standings_tx, standings_cancel).await;
    });

    // Spawn games source
    let games_source = GamesSource::new(
        client.clone(),
        games_rx,
        app.state.date_state.date.to_string(),
    );
    let games_tx = tx.clone();
    let games_cancel = cancel.clone();
    tokio::spawn(async move {
        Box::new(games_source).run(games_tx, games_cancel).await;
    });

    // Spawn boxscore source
    let boxscore_source = BoxscoreSource::new(client.clone(), boxscore_rx);
    let boxscore_tx = tx.clone();
    let boxscore_cancel = cancel.clone();
    tokio::spawn(async move {
        Box::new(boxscore_source)
            .run(boxscore_tx, boxscore_cancel)
            .await;
    });

    // Spawn play-by-play source
    let plays_source = PlaysSource::new(client.clone(), plays_rx);
    let plays_tx = tx.clone();
    let plays_cancel = cancel.clone();
    tokio::spawn(async move {
        Box::new(plays_source).run(plays_tx, plays_cancel).await;
    });

    // Spawn game story source
    let game_story_source = GameStorySource::new(client.clone(), game_story_rx);
    let game_story_tx = tx.clone();
    let game_story_cancel = cancel.clone();
    tokio::spawn(async move {
        Box::new(game_story_source)
            .run(game_story_tx, game_story_cancel)
            .await;
    });

    // Spawn team stats source
    let team_stats_source = TeamStatsSource::new(
        client.clone(),
        team_stats_rx,
        app.settings.favorite_team.unwrap_or_default(),
        app.state.date_state.year,
    );
    let team_stats_tx = tx.clone();
    let team_stats_cancel = cancel.clone();
    tokio::spawn(async move {
        Box::new(team_stats_source)
            .run(team_stats_tx, team_stats_cancel)
            .await;
    });

    // Spawn playoff bracket source
    let playoff_bracket_source =
        BracketSource::new(client.clone(), bracket_rx, app.state.date_state.year);
    let playoff_bracket_tx = tx.clone();
    let playoff_bracket_cancel = cancel.clone();
    tokio::spawn(async move {
        Box::new(playoff_bracket_source)
            .run(playoff_bracket_tx, playoff_bracket_cancel)
            .await;
    });

    // Spawn playoffs series source
    let playoff_bracket_source =
        SeriesSource::new(client.clone(), series_rx, app.state.date_state.year, None);
    let playoff_bracket_tx = tx.clone();
    let playoff_bracket_cancel = cancel.clone();
    tokio::spawn(async move {
        Box::new(playoff_bracket_source)
            .run(playoff_bracket_tx, playoff_bracket_cancel)
            .await;
    });

    // Spawn total-goals series source
    let total_goals_source = TotalGoalsSource::new(client.clone(), total_goals_rx);
    let total_goals_tx = tx.clone();
    let total_goals_cancel = cancel.clone();
    tokio::spawn(async move {
        Box::new(total_goals_source)
            .run(total_goals_tx, total_goals_cancel)
            .await;
    });

    // Spawn bracket series source
    let bracket_series_source = BracketSeriesSource::new(client.clone(), bracket_series_rx);
    let bracket_series_tx = tx.clone();
    let bracket_series_cancel = cancel.clone();
    tokio::spawn(async move {
        Box::new(bracket_series_source)
            .run(bracket_series_tx, bracket_series_cancel)
            .await;
    });

    // Spawn the season resolver
    let season_source = SeasonSource::new(client.clone(), app.state.date_state.date);
    let season_tx = tx.clone();
    let season_cancel = cancel.clone();
    tokio::spawn(async move {
        Box::new(season_source).run(season_tx, season_cancel).await;
    });

    // Spawn the game day poller
    let today_source = TodaySource::new(client, app.state.date_state.today);
    let today_tx = tx.clone();
    let today_cancel = cancel.clone();
    tokio::spawn(async move {
        Box::new(today_source).run(today_tx, today_cancel).await;
    });

    // Spawn terminal event reader
    let input_tx = tx.clone();
    let input_cancel = cancel.clone();
    tokio::spawn(async move {
        let mut reader = EventStream::new();
        loop {
            tokio::select! {
                _ = input_cancel.cancelled() => break,
                event = reader.next() => {
                    match event {
                        Some(Ok(Event::Key(key))) => {
                            let _ = input_tx.send(AppEvent::Input(key)).await;
                        }
                        Some(Err(e)) => {
                            log::error!("Terminal event error: {}", e);
                            break;
                        }
                        None => break,
                        _ => {}
                    }
                }
            }
        }
    });

    // Spawn tick timer
    let tick_tx = tx;
    let tick_cancel = cancel.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(200));
        loop {
            tokio::select! {
                _ = tick_cancel.cancelled() => break,
                _ = interval.tick() => {
                    let _ = tick_tx.send(AppEvent::Tick).await;
                }
            }
        }
    });

    // Main event loop
    loop {
        terminal.draw(|f| ui::render::render(f, app))?;

        if let Some(event) = rx.recv().await {
            app.state.handle_event(event);
            if app.state.should_quit {
                break;
            }
        } else {
            break;
        }
    }

    Ok(())
}
