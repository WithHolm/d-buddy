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
use std::sync::{Arc, Mutex};

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
use tokio::{sync::mpsc, time::Instant};

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
    /// Set the maximum number of messages to keep in memory (rolling window)
    #[arg(long)]
    max_messages: Option<usize>,
}

// Main asynchronous entry point of the application
#[tokio::main]
async fn main() -> Result<()> {
    #[cfg(target_os = "linux")]
    check_clipboard_utilities();

    let args = Args::parse();

    let _log_guard = if args.log {
        tracing_log::LogTracer::init().expect("Failed to set logger");
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
    if let Some(max_msgs) = args.max_messages {
        config.max_messages = max_msgs;
    }
    config.enable_debug_ui = args.debug_ui;

    let (session_sender, session_receiver) = mpsc::channel(1024);
    let (system_sender, system_receiver) = mpsc::channel(1024);

    bus::dbus_listener(BusType::Session, session_sender).await?;
    bus::dbus_listener(BusType::System, system_sender).await?;

    let mut app = App::new(session_receiver, system_receiver);
    app.initialize_static_ui_elements(&config);

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

#[cfg(target_os = "linux")]
fn check_clipboard_utilities() {
    let utilities = ["xclip", "xsel", "wl-copy"];
    let is_any_found = utilities.iter().any(|util| {
        std::process::Command::new(util)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok()
    });

    if !is_any_found {
        eprintln!("Error: Clipboard utility not found.");
        eprintln!("d-buddy requires 'xclip', 'xsel', or 'wl-copy' to be installed for clipboard functionality on Linux.");
        eprintln!("Please install one of them using your system's package manager (e.g., 'sudo pacman -S wl-copy').");
        std::process::exit(1);
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

    let clipboard_arc = Arc::new(Mutex::new(Clipboard::new().unwrap()));

    let tick_rate = Duration::from_millis(24);
    let min_wait = Duration::from_millis(2);

    loop {
        tracing::debug!("Start of loop: selected = {:?}", app.list_state.selected());
        let loop_timer = Instant::now();
        let _main_loop_span = tracing::debug_span!("main_loop").entered();

        let mut new_messages_received = false;
        while let Ok(item) = app.poll_session_messages() {
            app.add_session_item(item);
            new_messages_received = true;
        }
        if app.session_items_len() > config.max_messages {
            let excess = app.session_items_len() - config.max_messages;
            app.drain_session_items(0..excess);
        }

        while let Ok(item) = app.poll_system_messages() {
            app.add_system_item(item);
            new_messages_received = true;
        }
        if app.system_items_len() > config.max_messages {
            let excess = app.system_items_len() - config.max_messages;
            app.drain_system_items(0..excess);
        }

        let session_count = app.session_items_len();
        let system_count = app.system_items_len();
        let both_count = session_count + system_count;

        // Create a scope to ensure the lock is released before drawing

        {
            let _processing_span = tracing::info_span!("message_processing").entered();
            let processed_messages: Vec<Item> = match app.stream {
                BusType::Session => app.get_session_items().to_vec(),
                BusType::System => app.get_system_items().to_vec(),
                BusType::Both => {
                    let session_items = app.get_session_items();
                    let system_items = app.get_system_items();
                    let mut combined =
                        Vec::with_capacity(session_items.len() + system_items.len());
                    combined.extend(session_items.iter().cloned());
                    combined.extend(system_items.iter().cloned());
                    combined.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
                    // Also trim the combined view if it exceeds max_messages
                    if combined.len() > config.max_messages {
                        let start = combined.len() - config.max_messages;
                        combined.drain(0..start);
                    }
                    combined
                }
            };

            // A future optimization would be to use references and avoid this clone.
            tracing::debug!(
                "Filtering: processed_messages.len() = {}",
                processed_messages.len()
            );

            let filter_text = app.input.value();

            {
                let _filter_span = tracing::info_span!("filtering").entered();
                tracing::debug!(
                    "Filtering: filter_text = {:?}, filter_criteria = {:?}",
                    filter_text,
                    app.filter_criteria
                );
                app.filtered_and_sorted_items = processed_messages
                    .into_iter() // Use into_iter to consume the vector
                    .filter(|item| match app.mode {
                        Mode::ConversationView => {
                            if let Some(conversation_serial) = &app.conversation_serial {
                                item.serial == *conversation_serial
                                    || item.reply_serial == *conversation_serial
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
                    .collect();
                tracing::debug!(
                    "Filtering: app.filtered_and_sorted_items.len() after filter = {}",
                    app.filtered_and_sorted_items.len()
                );
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

            if new_messages_received && app.follow {
                if !app.filtered_and_sorted_items.is_empty() {
                    let new_index = app.filtered_and_sorted_items.len() - 1;
                    app.list_state.select(Some(new_index));
                }
            } else if app.list_state.selected().is_none() && !app.filtered_and_sorted_items.is_empty() {
                // BUGFIX: Ensure an item is selected by default if the list is not empty
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
            if event::handle_event(app, config, event, clipboard_arc.clone()).await? {
                break;
            }
        }
    }

    Ok(())
}
