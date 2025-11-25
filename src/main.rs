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
use chrono::{DateTime, Local};

use clap::Parser;
use crossterm::event::EventStream;
use futures::stream::StreamExt; // For extending stream functionality, used with zbus MessageStream
use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::ListItem;

// UI widgets
use bus::{BusType, GroupingType, Item};
use std::{
    io::{self, stdout},
    time::Duration,
};

/// A simple TUI for browsing D-Bus messages.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Run in check mode without launching the TUI
    #[arg(long)]
    check: bool,
}

// Main asynchronous entry point of the application
#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let config = Config::default();

    let mut app = App::default();
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

async fn run<'a>(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,

    app: &mut App<'a>,

    config: &Config,
) -> Result<()> {
    let mut event_stream = EventStream::new();

    let mut clipboard = Clipboard::new().unwrap();

    let tick_rate = Duration::from_millis(250);

    loop {
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
            let all_messages = match app.stream {
                BusType::Session | BusType::System => {
                    app.messages.get(&app.stream).unwrap().lock().await.clone()
                }

                BusType::Both => {
                    let mut combined_messages: Vec<Item> = Vec::new();

                    if let Some(session_arc) = app.messages.get(&BusType::Session) {
                        combined_messages.extend(session_arc.lock().await.iter().cloned());
                    }

                    if let Some(system_arc) = app.messages.get(&BusType::System) {
                        combined_messages.extend(system_arc.lock().await.iter().cloned());
                    }

                    combined_messages.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

                    combined_messages
                }
            };

            let filter_text = app.input.value();

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
                                let item_field_value = match field.as_str() {
                                    "sender" => &item.sender,

                                    "member" => &item.member,

                                    "path" => &item.path,

                                    "serial" => &item.serial,

                                    "reply_serial" => &item.reply_serial,

                                    _ => {
                                        passes_field_filters = false;

                                        break;
                                    }
                                };

                                if !item_field_value.contains(value) {
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

            let mut last_group_keys_composite: Option<String> = None;

            let now = Local::now(); // Get current time once per loop iteration

            app.list_items = app
                .filtered_and_sorted_items
                .iter()
                .flat_map(|item| {
                    let mut items_to_render = Vec::new();
                    let mut current_group_keys_vec = Vec::new();
                    let mut is_grouped = false;

                    for key in &app.grouping_keys {
                        if key == &bus::GroupingType::None {
                            continue;
                        }
                        is_grouped = true;
                        let group_component = match key {
                            bus::GroupingType::Sender => item.app_name.clone(),
                            bus::GroupingType::Member => item.member.clone(),
                            bus::GroupingType::Path => item.path.clone(),
                            bus::GroupingType::Serial => item.serial.clone(),
                            bus::GroupingType::None => unreachable!(),
                        };
                        current_group_keys_vec.push(group_component);
                    }
                    let current_group_keys_composite = current_group_keys_vec.join("::");

                    if is_grouped
                        && last_group_keys_composite.as_ref() != Some(&current_group_keys_composite)
                    {
                        let header_spans = vec![Span::styled(
                            current_group_keys_composite.clone(),
                            Style::default().fg(config.color_grouping_header).bold(),
                        )];
                        items_to_render.push(ListItem::new(Line::from(header_spans)));
                        last_group_keys_composite = Some(current_group_keys_composite);
                    }

                    let indent = if is_grouped { "  " } else { "" };
                    let dt: DateTime<Local> = item.timestamp.into();
                    let timestamp = if app.use_relative_time {
                        let duration = now.signed_duration_since(dt).abs();
                        if duration.num_seconds() < 60 {
                            if duration.num_seconds() < 10 {
                                format!("{}s", duration.num_seconds())
                            } else {
                                format!("{}+s", (duration.num_seconds() / 10) * 10)
                            }
                        } else if duration.num_minutes() < 60 {
                            format!("{}m", duration.num_minutes())
                        } else if duration.num_hours() < 24 {
                            format!("{}h", duration.num_hours())
                        } else if duration.num_days() < 365 {
                            format!("{}d", duration.num_days())
                        } else {
                            format!("{}y", duration.num_days() / 365)
                        }
                    } else {
                        dt.format("%H:%M:%S%.3f").to_string()
                    };

                    let elapsed_seconds = now.signed_duration_since(dt).num_seconds().max(0) as u64;
                    let total_fade_seconds = 60;
                    let ticker_color = if elapsed_seconds < total_fade_seconds {
                        get_fading_color(config.color_ticker, elapsed_seconds, total_fade_seconds)
                    } else {
                        Color::DarkGray
                    };
                    let mut ticker_span = Span::raw("");
                    if app.enable_lighting_strike {
                        ticker_span = Span::styled("⚡", Style::default().fg(ticker_color));
                    }

                    let mut spans = vec![
                        Span::raw(indent),
                        ticker_span,
                        Span::raw(" ["),
                        Span::styled(
                            timestamp,
                            if app.show_details {
                                Style::default().fg(config.color_timestamp_details)
                            } else {
                                Style::default().fg(config.color_timestamp_normal)
                            },
                        ),
                        Span::raw("]"),
                        Span::raw(" "),
                    ];

                    let sender_info = item.sender_display();
                    let receiver_info = item.receiver_display();

                    let mut new_spans = vec![];
                    if item.is_reply {
                        new_spans.push(Span::raw(" ↩ "));
                    } else {
                        new_spans.push(Span::raw("   "));
                    }

                    new_spans.push(Span::styled(
                        sender_info,
                        if app.show_details {
                            Style::default().fg(config.color_sender_details)
                        } else {
                            Style::default().fg(config.color_sender_normal)
                        },
                    ));

                    if !receiver_info.is_empty() {
                        new_spans.push(Span::raw(" -> "));
                        new_spans.push(Span::styled(
                            receiver_info,
                            if app.show_details {
                                Style::default().fg(config.color_sender_details)
                            } else {
                                Style::default().fg(config.color_sender_normal)
                            },
                        ));
                    }

                    new_spans.push(Span::raw(" "));
                    new_spans.push(Span::styled(
                        item.member.clone(),
                        if app.show_details {
                            Style::default().fg(config.color_member_details)
                        } else {
                            Style::default().fg(config.color_member_normal)
                        },
                    ));
                    new_spans.push(Span::raw("@"));
                    new_spans.push(Span::styled(
                        item.path.clone(),
                        if app.show_details {
                            Style::default().fg(config.color_path_details)
                        } else {
                            Style::default().fg(config.color_path_normal)
                        },
                    ));

                    spans.extend(new_spans);

                    items_to_render.push(ListItem::new(Line::from(spans)));

                    items_to_render
                })
                .collect();

            // BUGFIX: Ensure an item is selected by default if the list is not empty

            if app.list_state.selected().is_none() && !app.list_items.is_empty() {
                app.list_state.select(Some(0));
            }
        }

        terminal.draw(|f| ui::ui(f, app, config, session_count, system_count, both_count))?;

        let event_ready = tokio::time::timeout(tick_rate, event_stream.next()).await;

        if let Ok(Some(Ok(event))) = event_ready {
            if event::handle_event(app, config, event, &mut clipboard).await? {
                break;
            }
        }
    }

    Ok(())
}
