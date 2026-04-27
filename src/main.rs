mod driver;
mod app;
mod ui;

use std::env;
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
    let args: Vec<String> = env::args().collect();
    let mut config = Config::default();
    if args.len() > 1 {
        config.device_paths = args[1..].to_vec();
    }

    let mut psus = Psu::setup_all(&config);

    if psus.is_empty() {
        eprintln!("Error: No serial devices opened.");
        eprintln!("Usage: {} [/dev/ttyUSB0 ...]", args.first().map(String::as_str).unwrap_or("corsair-top"));
        eprintln!();
        eprintln!("If /dev/ttyUSB0 doesn't exist, bind the cp210x driver to your AX1500i:");
        eprintln!("  sudo modprobe cp210x");
        eprintln!("  echo \"1b1c 1c02\" | sudo tee /sys/bus/usb-serial/drivers/cp210x/new_id");
        return Ok(());
    }

    println!("Opened {} PSU(s). Initializing dongle(s)...", psus.len());

    for psu in psus.iter_mut() {
        psu.setup_dongle();
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(psus.len());

    let readings: Vec<_> = psus.iter_mut().map(|psu| psu.read_all()).collect();
    app.is_ax1600i = readings.iter().any(|r| r.name.contains("AX1600"));
    app.update(readings);

    let result = run_loop(&mut terminal, &mut app, &mut psus);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

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
        terminal.draw(|frame| {
            ui::draw(frame, app);
        })?;

        let tick_rate = Duration::from_millis(app.tick_rate_ms);
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') => app.should_quit = true,
                        KeyCode::Esc => app.should_quit = true,
                        KeyCode::Char('=') | KeyCode::Char('+') => app.increase_tick_rate(),
                        KeyCode::Char('-') => app.decrease_tick_rate(),
                        _ => {}
                    }
                }
            }
        }

        if app.should_quit {
            return Ok(());
        }

        if last_tick.elapsed() >= tick_rate {
            let readings: Vec<_> = psus.iter_mut().map(|psu| psu.read_all()).collect();
            app.update(readings);
            last_tick = Instant::now();
        }
    }
}
