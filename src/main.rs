mod bus;
mod config;
mod event;
mod state;
mod ui;

use config::Config;
use state::{App, Mode};

// Import necessary crates and modules
use anyhow::Result; // For simplified error handling
use arboard::Clipboard; // For clipboard access

use clap::Parser;
use crossterm::event::EventStream;
use futures::stream::StreamExt; // For extending stream functionality, used with zbus MessageStream
use ratatui::prelude::*;

// UI widgets
use bus::{BusType, Item};
use std::{
    env,
    io::{self, stdout},
    time::Duration,
};
use tokio::time::Instant;

use tracing::instrument;
use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};

/// A simple TUI for browsing D-Bus messages.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Run in check mode without launching the TUI
    #[arg(long)]
    check: bool,
    /// Enable logging to d-buddy.log
    #[arg(long)]
    log: bool,
    /// Enable debug UI elements
    #[arg(long)]
    debug_ui: bool,
}

// Main asynchronous entry point of the application
#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let _log_guard = if args.log {
        let file_appender = tracing_appender::rolling::daily(".", "d-buddy.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
        let subscriber = tracing_subscriber::fmt()
            .with_writer(non_blocking)
            .with_ansi(false)
            .with_span_events(FmtSpan::CLOSE)
            .with_env_filter(
                EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
            )
            .finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("setting default subscriber failed");

        tracing::info!("Starting d-buddy...");
        tracing::info!("App path: {}", env::current_exe().unwrap().display());
        tracing::info!("Args: {:?}", args);
        tracing::info!(
            "Log level: {}",
            env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string())
        );

        Some(guard)
    } else {
        None
    };

    let mut config = Config::default();
    config.enable_debug_ui = args.debug_ui;

    let mut app = App::default();
    app.initialize_static_ui_elements(&config);
    let session_messages = bus::dbus_listener(BusType::Session).await?;
    let system_messages = bus::dbus_listener(BusType::System).await?;
    app.messages.insert(BusType::Session, session_messages);
    app.messages.insert(BusType::System, system_messages);

    if args.check {
        println!("Check mode: Setup successful. App initialized and listeners started.");
        tokio::time::sleep(Duration::from_millis(500)).await;
        println!("Check finished.");
        Ok(())
    } else {
        let mut terminal = setup_terminal()?;
        run(&mut terminal, &mut app, &config).await?;
        restore_terminal()?;
        Ok(())
    }
}

// Sets up the terminal for TUI mode
fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    let mut stdout = stdout();
    // Enable raw mode to capture individual key presses
    crossterm::terminal::enable_raw_mode()?;
    // Enter the alternate screen buffer, so the TUI doesn't mess up the main terminal content
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    // Create a ratatui backend for Crossterm
    let backend = CrosstermBackend::new(stdout);
    // Create and return a new terminal instance
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

// Restores the terminal to its original state
fn restore_terminal() -> Result<()> {
    // Leave the alternate screen buffer
    crossterm::execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen)?;
    // Disable raw mode
    crossterm::terminal::disable_raw_mode()?;
    Ok(())
}

fn get_fading_color(base_color: Color, elapsed_seconds: u64, total_seconds: u64) -> Color {
    let elapsed_fraction = elapsed_seconds as f32 / total_seconds as f32;

    // Assuming base_color is RGB for interpolation

    if let Color::Rgb(r_base, g_base, b_base) = base_color {
        // Fade to black (0,0,0) or dark gray

        let r = (r_base as f32 * (1.0 - elapsed_fraction)).max(0.0) as u8;

        let g = (g_base as f32 * (1.0 - elapsed_fraction)).max(0.0) as u8;

        let b = (b_base as f32 * (1.0 - elapsed_fraction)).max(0.0) as u8;

        Color::Rgb(r, g, b)
    } else {
        // Fallback for other color types, or if you only use RGB

        if elapsed_fraction < (total_seconds as f32 / 2.0) {
            // Keep original color for first half

            base_color
        } else {
            Color::DarkGray // Fade to dark gray for second half
        }
    }
}

// The main application event loop, now fully asynchronous
#[instrument(skip_all)]
async fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    config: &Config,
) -> Result<()> {
    let mut event_stream = EventStream::new();

    let mut clipboard = Clipboard::new().unwrap();

    let tick_rate = Duration::from_millis(24);
    let min_wait = Duration::from_millis(2);

    loop {
        tracing::debug!("Start of loop: selected = {:?}", app.list_state.selected());
        let loop_timer = Instant::now();
        let _main_loop_span = tracing::debug_span!("main_loop").entered();
        let session_count = if let Some(arc) = app.messages.get(&BusType::Session) {
            arc.lock().await.len()
        } else {
            0
        };

        let system_count = if let Some(arc) = app.messages.get(&BusType::System) {
            arc.lock().await.len()
        } else {
            0
        };

        let both_count = session_count + system_count;

        // Create a scope to ensure the lock is released before drawing

        {
            let _processing_span = tracing::info_span!("message_processing").entered();
            let all_messages = match app.stream {
                BusType::Session | BusType::System => {
                    let _message_collection_span =
                        tracing::info_span!("message_collection_single_bus").entered();
                    app.messages.get(&app.stream).unwrap().lock().await.clone()
                }

                BusType::Both => {
                    let mut combined_messages: Vec<Item> = Vec::new();

                    if let Some(session_arc) = app.messages.get(&BusType::Session) {
                        let _session_extend_span =
                            tracing::info_span!("message_collection_extend_session").entered();
                        combined_messages.extend(session_arc.lock().await.iter().cloned());
                    }

                    if let Some(system_arc) = app.messages.get(&BusType::System) {
                        let _system_extend_span =
                            tracing::info_span!("message_collection_extend_system").entered();
                        combined_messages.extend(system_arc.lock().await.iter().cloned());
                    }

                    {
                        let _combined_sort_span =
                            tracing::info_span!("message_collection_combined_sort").entered();
                        combined_messages.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
                    }
                    combined_messages
                }
            };

            let filter_text = app.input.value();

            {
                let _filter_span = tracing::info_span!("filtering").entered();
                app.filtered_and_sorted_items = all_messages
                    .iter()
                    .filter(|item| match app.mode {
                        Mode::ThreadView => {
                            if let Some(thread_serial) = &app.thread_serial {
                                item.serial == *thread_serial || item.reply_serial == *thread_serial
                            } else {
                                false
                            }
                        }

                        _ => {
                            let mut passes_field_filters = true;

                            if !app.filter_criteria.is_empty() {
                                for (field, value) in &app.filter_criteria {
                                    let item_field_value: std::borrow::Cow<'_, str> =
                                        match field.as_str() {
                                            "sender" => item.sender_display(),
                                            "member" => item.member.as_str().into(),
                                            "path" => item.path.as_str().into(),
                                            "serial" => item.serial.as_str().into(),
                                            "reply_serial" => item.reply_serial.as_str().into(),
                                            _ => {
                                                passes_field_filters = false;
                                                "".into() // Return an empty Cow
                                            }
                                        };
                                    if passes_field_filters && !item_field_value.contains(value) {
                                        passes_field_filters = false;

                                        break;
                                    }
                                }
                            }

                            let passes_general_filter = filter_text.is_empty()
                                || item.sender.contains(filter_text)
                                || item.member.contains(filter_text)
                                || item.path.contains(filter_text);

                            passes_field_filters && passes_general_filter
                        }
                    })
                    .cloned()
                    .collect();
            }
            {
                let _sort_span = tracing::info_span!("sorting").entered();
                app.filtered_and_sorted_items.sort_by(|a, b| {
                    let mut cmp = std::cmp::Ordering::Equal;

                    for key in &app.grouping_keys {
                        cmp = match key {
                            bus::GroupingType::Sender => a.app_name.cmp(&b.app_name),

                            bus::GroupingType::Member => a.member.cmp(&b.member),

                            bus::GroupingType::Path => a.path.cmp(&b.path),

                            bus::GroupingType::Serial => a.serial.cmp(&b.serial),

                            bus::GroupingType::None => std::cmp::Ordering::Equal,
                        };

                        if cmp != std::cmp::Ordering::Equal {
                            break;
                        }
                    }

                    if cmp == std::cmp::Ordering::Equal {
                        a.timestamp.cmp(&b.timestamp)
                    } else {
                        cmp
                    }
                });
            }

            // BUGFIX: Ensure an item is selected by default if the list is not empty

            if app.list_state.selected().is_none() && !app.filtered_and_sorted_items.is_empty() {
                let _list_selection_span = tracing::info_span!("list_selection_init").entered();
                app.list_state.select(Some(0));
            }
        }
        {
            let _draw_span = tracing::debug_span!("drawing_ui").entered();
            terminal.draw(|f| {
                let temp_filtered_items = std::mem::take(&mut app.filtered_and_sorted_items);
                ui::ui(
                    f,
                    app,
                    config,
                    session_count,
                    system_count,
                    both_count,
                    &temp_filtered_items[..],
                );
                app.filtered_and_sorted_items = temp_filtered_items;
            })?;
        }

        //check if any key is pressed
        let event_ready;
        {
            let elapsed = loop_timer.elapsed();
            // Calculate dynamic timeout with minimum wait
            let timeout = if elapsed >= tick_rate {
                // We are behind schedule, but still yield briefly
                min_wait
            } else {
                // Normal case: leftover time + safety minimum
                (tick_rate - elapsed).max(min_wait)
            };

            let _event_polling_span = tracing::debug_span!("waiting_for_user_keypress").entered();
            event_ready = tokio::time::timeout(timeout, event_stream.next()).await;
        }

        // handle any keypress
        if let Ok(Some(Ok(event))) = event_ready {
            let _event_handling_span = tracing::info_span!("handling_user_input").entered();
            if event::handle_event(app, config, event, &mut clipboard).await? {
                break;
            }
        }
    }

    Ok(())
}
