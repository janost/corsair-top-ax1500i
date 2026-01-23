mod driver;
mod app;
mod ui;

use std::io;
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    execute,
};
use ratatui::prelude::*;

use driver::{Psu, Config};
use app::App;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize USB context
    let context = rusb::Context::new()?;

    // Find and set up PSUs
    let config = Config::default();
    let mut psus = Psu::setup_all(&context, config);

    if psus.is_empty() {
        eprintln!("Error: No Corsair AX1600i PSUs found.");
        eprintln!("Make sure the PSU is connected via USB and you have appropriate permissions.");
        eprintln!("You may need to run with sudo or set up udev rules.");
        return Ok(());
    }

    println!("Found {} PSU(s). Initializing...", psus.len());

    // Set up dongles
    for psu in psus.iter_mut() {
        psu.setup_dongle();
    }

    // Set up terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new(psus.len());

    // Initial reading
    let readings: Vec<_> = psus.iter_mut().map(|psu| psu.read_all()).collect();
    app.is_ax1600i = readings.iter().any(|r| r.name.contains("AX1600i"));
    app.update(readings);

    // Main event loop
    let result = run_loop(&mut terminal, &mut app, &mut psus);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    // Release PSUs
    for psu in psus.iter_mut() {
        psu.release();
    }

    if let Err(err) = result {
        eprintln!("Error: {}", err);
    }

    Ok(())
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    psus: &mut Vec<Psu>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut last_tick = Instant::now();

    loop {
        // Draw UI
        terminal.draw(|frame| {
            ui::draw(frame, app);
        })?;

        // Calculate timeout until next tick
        let tick_rate = Duration::from_millis(app.tick_rate_ms);
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        // Poll for events
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') => {
                            app.should_quit = true;
                        }
                        KeyCode::Esc => {
                            app.should_quit = true;
                        }
                        KeyCode::Char('=') | KeyCode::Char('+') => {
                            app.increase_tick_rate();
                        }
                        KeyCode::Char('-') => {
                            app.decrease_tick_rate();
                        }
                        _ => {}
                    }
                }
            }
        }

        if app.should_quit {
            return Ok(());
        }

        // Update readings on tick
        if last_tick.elapsed() >= tick_rate {
            let readings: Vec<_> = psus.iter_mut().map(|psu| psu.read_all()).collect();
            app.update(readings);
            last_tick = Instant::now();
        }
    }
}
