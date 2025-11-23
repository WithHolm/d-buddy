mod bus;
mod ui;

// Import necessary crates and modules
use anyhow::Result; // For simplified error handling
use arboard::Clipboard; // For clipboard access
use chrono::{DateTime, Local};
use clap::Parser;
use crossterm::event::{Event, EventStream, KeyCode}; // For handling terminal events like key presses
use futures::stream::StreamExt; // For extending stream functionality, used with zbus MessageStream
use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::{ListItem, ListState};
use std::collections::HashMap;
use std::sync::Arc;
// UI widgets
use bus::{BusType, Item};
use std::{
    io::{self, stdout},
    thread::sleep,
    time::Duration,
}; // Standard I/O for terminal operations
use tokio::fs; // Asynchronous multi-producer, single-consumer channel for message passing
use tui_input::{backend::crossterm as input_backend, Input}; // For handling text input in the TUI
use zbus::zvariant::{Structure, Value};

/// A simple TUI for browsing D-Bus messages.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Run in check mode without launching the TUI
    #[arg(long)]
    check: bool,
}

pub struct Config {
    pub color_dict: Color,
    pub color_struct: Color,
    pub color_default_stripe: Color,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            color_dict: Color::Rgb(20, 20, 40),   // Dark Blue
            color_struct: Color::Rgb(40, 20, 40), // Dark Magenta
            color_default_stripe: Color::DarkGray,
        }
    }
}

// Enum to define the current operating mode of the application
enum Mode {
    Normal,              // Default mode for browsing D-Bus messages
    Filtering,           // Mode for entering a filter string
    AutoFilterSelection, // Mode for selecting autofilter field
    ThreadView,          // Mode for viewing a specific message thread
}

// Main application struct holding all the state
struct App<'a> {
    stream: BusType,
    messages: HashMap<BusType, Arc<tokio::sync::Mutex<Vec<Item>>>>,
    filtered_and_sorted_items: Vec<Item>,
    list_items: Vec<ListItem<'a>>,
    list_state: ListState, // State of the message list widget (e.g., selected item)
    show_details: bool,    // Flag to indicate if message details popup should be shown
    mode: Mode,            // Current operating mode (Normal or Filtering)
    input: Input,          // Input buffer for the filtering text
    detail_text: Text<'static>, // The formatted string for the currently viewed detail
    detail_scroll: u16,    // The vertical scroll offset for the detail view
    status_message: String, // A temporary message to show in the status bar
    thread_serial: Option<String>,
    detail_scroll_request: Option<i32>,
    filter_criteria: HashMap<String, String>,
}

// Default implementation for the App struct
impl Default for App<'_> {
    fn default() -> Self {
        App {
            stream: BusType::Session,
            messages: HashMap::new(), // Initialize with an empty list of messages
            filtered_and_sorted_items: Vec::new(),
            list_items: Vec::new(),
            list_state: ListState::default(), // Default list state (no item selected)
            show_details: false,              // Details popup is hidden by default
            mode: Mode::Normal,               // Start in Normal mode
            input: Input::default(),          // Empty input buffer
            detail_text: Text::default(),     // No detail text initially
            detail_scroll: 0,                 // Start with no scroll
            status_message: String::new(),    // No status message initially
            thread_serial: None,
            detail_scroll_request: None,
            filter_criteria: HashMap::new(),
        }
    }
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

            app.filtered_and_sorted_items
                .sort_by(|a, b| a.sender.cmp(&b.sender).then(a.timestamp.cmp(&b.timestamp)));

            let mut last_sender: Option<String> = None;
            app.list_items = app
                .filtered_and_sorted_items
                .iter()
                .map(|item| {
                    let indent = if last_sender.as_ref() == Some(&item.sender) {
                        "  "
                    } else {
                        last_sender = Some(item.sender.clone());
                        ""
                    };
                    let dt: DateTime<Local> = item.timestamp.into();
                    let timestamp = dt.format("%H:%M:%S%.3f").to_string();
                    let line = Line::from(vec![
                        Span::raw(indent),
                        Span::raw("["),
                        Span::styled(
                            timestamp,
                            if app.show_details {
                                Style::default().fg(Color::White)
                            } else {
                                Style::default().fg(Color::Yellow)
                            },
                        ),
                        Span::raw("]"),
                        if app.stream == BusType::Both {
                            match item.stream_type {
                                BusType::Session => {
                                    Span::styled("[sess]", Style::default().fg(Color::Cyan))
                                }
                                BusType::System => {
                                    Span::styled("[syst]", Style::default().fg(Color::LightMagenta))
                                }
                                _ => Span::raw(""), // Should not happen in this context
                            }
                        } else {
                            Span::raw("")
                        },
                        if item.is_reply {
                            Span::raw(" ↩ ")
                        } else {
                            Span::raw(" ")
                        },
                        Span::raw("sender: "),
                        Span::styled(
                            item.sender.clone(),
                            if app.show_details {
                                Style::default().fg(Color::White)
                            } else {
                                Style::default().fg(Color::Green)
                            },
                        ),
                        Span::raw(", member: "),
                        Span::styled(
                            item.member.clone(),
                            if app.show_details {
                                Style::default().fg(Color::White)
                            } else {
                                Style::default().fg(Color::Blue)
                            },
                        ),
                        Span::raw(", path: "),
                        Span::styled(
                            item.path.clone(),
                            if app.show_details {
                                Style::default().fg(Color::White)
                            } else {
                                Style::default().fg(Color::Magenta)
                            },
                        ),
                    ]);
                    ListItem::new(line)
                })
                .collect();
            
            // BUGFIX: Ensure an item is selected by default if the list is not empty
            if app.list_state.selected().is_none() && !app.list_items.is_empty() {
                app.list_state.select(Some(0));
            }
        }

        terminal.draw(|f| ui::ui(f, app))?;

        let event_ready = tokio::time::timeout(tick_rate, event_stream.next()).await;

        if let Ok(Some(Ok(event))) = event_ready {
            match app.mode {
                Mode::Normal => {
                    if let Event::Key(key) = event {
                        if !app.status_message.is_empty() {
                            app.status_message.clear();
                        }

                        match key.code {
                            KeyCode::Char('q') => {
                                if !app.show_details {
                                    break;
                                }
                                app.show_details = false;
                            }
                            KeyCode::Tab => {
                                app.stream = match app.stream {
                                    BusType::Session => BusType::System,
                                    BusType::System => BusType::Both,
                                    BusType::Both => BusType::Session,
                                };
                                app.list_state.select(None); // Reset selection
                            }
                            KeyCode::Char('x') => {
                                if let Some(messages_arc) = app.messages.get(&app.stream) {
                                    let mut messages = messages_arc.lock().await;
                                    messages.clear();
                                    app.list_state.select(None);
                                }
                            }
                            KeyCode::Char('a') => {
                                if app.list_state.selected().is_some() {
                                    app.mode = Mode::AutoFilterSelection;
                                }
                            }
                            KeyCode::Char('t') => {
                                if let Some(selected) = app.list_state.selected() {
                                    if let Some(item) = app.filtered_and_sorted_items.get(selected)
                                    {
                                        app.thread_serial = Some(item.serial.clone());
                                        app.mode = Mode::ThreadView;
                                    }
                                }
                            }
                            KeyCode::Char('f') => app.mode = Mode::Filtering,
                            KeyCode::Up => {
                                if !app.list_items.is_empty() {
                                    let i = match app.list_state.selected() {
                                        Some(i) => i.saturating_sub(1),
                                        None => 0,
                                    };
                                    app.list_state.select(Some(i));
                                    if app.show_details {
                                        update_detail_text(app, config);
                                    }
                                }
                            }
                            KeyCode::Down => {
                                if !app.list_items.is_empty() {
                                    let i = match app.list_state.selected() {
                                        Some(i) => (i + 1).min(app.list_items.len() - 1),
                                        None => 0,
                                    };
                                    app.list_state.select(Some(i));
                                    if app.show_details {
                                        update_detail_text(app, config);
                                    }
                                }
                            }
                            KeyCode::Char('s') | KeyCode::Char(' ') => {
                                if app.show_details {
                                    app.show_details = false;
                                } else {
                                    update_detail_text(app, config);
                                    app.show_details = true;
                                }
                            }
                            KeyCode::Esc => {
                                if app.show_details {
                                    app.show_details = false;
                                }
                            }
                            KeyCode::Char('r') => {
                                if let Some(selected) = app.list_state.selected() {
                                    if let Some(item) = app.filtered_and_sorted_items.get(selected)
                                    {
                                        let bus_type = match app.stream {
                                            BusType::Session => "--session",
                                            BusType::System => "--system",
                                            BusType::Both => "--session", // Default
                                        };
                                        let command = format!(
                                            "dbus-send {} --dest={} {} <interface>.<member>",
                                            bus_type, item.sender, item.path
                                        );
                                        match clipboard.set_text(command.clone()) {
                                            Ok(_) => {
                                                app.status_message =
                                                    format!("Copied to clipboard: {}", command);
                                            }
                                            Err(e) => {
                                                app.status_message =
                                                    format!("Failed to copy to clipboard: {}", e);
                                            }
                                        }
                                    }
                                }
                            }
                            KeyCode::Char('c') => {
                                if app.show_details {
                                    let text_to_copy_text = app.detail_text.clone();
                                    let text_to_copy = text_to_copy_text.to_string();
                                    let file_path = "/tmp/d-buddy-details.txt";
                                    let file_write_status =
                                        match fs::write(file_path, text_to_copy.as_bytes()).await {
                                            Ok(_) => format!("Saved to {}", file_path),
                                            Err(e) => format!("Failed to save to file: {}", e),
                                        };
                                    let clipboard_status = match clipboard.set_text(text_to_copy) {
                                        Ok(_) => {
                                            sleep(Duration::from_millis(100));
                                            "Copied to clipboard!".to_string()
                                        }
                                        Err(e) => format!("Copy failed: {}", e),
                                    };
                                    app.status_message =
                                        format!("{} | {}", file_write_status, clipboard_status);
                                }
                            }
                            KeyCode::Char('j') => {
                                if app.show_details {
                                    app.detail_scroll_request = Some(1);
                                }
                            }
                            KeyCode::Char('k') => {
                                if app.show_details {
                                    app.detail_scroll_request = Some(-1);
                                }
                            }
                            KeyCode::PageDown => {
                                if app.show_details {
                                    app.detail_scroll_request = Some(10);
                                }
                            }
                            KeyCode::PageUp => {
                                if app.show_details {
                                    app.detail_scroll_request = Some(-10);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Mode::Filtering => {
                    if let Event::Key(key) = event {
                        match key.code {
                            KeyCode::Enter => {
                                let input_value = app.input.value().to_string();
                                if input_value.is_empty() {
                                    // Clear all field filters and general text filter
                                    app.filter_criteria.clear();
                                    app.input.reset();
                                } else if let Some((field, value)) = input_value.split_once('=') {
                                    if value.is_empty() {
                                        // Clear a specific field filter
                                        app.filter_criteria.remove(field);
                                    } else {
                                        // Set a specific field filter
                                        app.filter_criteria
                                            .insert(field.to_string(), value.to_string());
                                    }
                                    app.input.reset(); // Clear input box after applying field filter
                                } else {
                                    // Treat as general text filter, clear field filters
                                    app.filter_criteria.clear();
                                    // app.input is already set with the general text, no reset here
                                }
                                app.mode = Mode::Normal;
                            }
                            KeyCode::Esc => {
                                app.input.reset();
                                app.filter_criteria.clear(); // Clear all field filters
                                app.mode = Mode::Normal;
                            }
                            _ => {
                                input_backend::to_input_request(&event)
                                    .and_then(|req| app.input.handle(req));
                            }
                        }
                    }
                }
                Mode::AutoFilterSelection => {
                    if let Event::Key(key) = event {
                        if let Some(selected) = app.list_state.selected() {
                            if let Some(item) = app.filtered_and_sorted_items.get(selected) {
                                match key.code {
                                    KeyCode::Char('s') => {
                                        app.input = Input::from(item.sender.as_str());
                                        app.mode = Mode::Filtering;
                                    }
                                    KeyCode::Char('m') => {
                                        app.input = Input::from(item.member.as_str());
                                        app.mode = Mode::Filtering;
                                    }
                                    KeyCode::Char('p') => {
                                        app.input = Input::from(item.path.as_str());
                                        app.mode = Mode::Filtering;
                                    }
                                    KeyCode::Char('r') => {
                                        app.input = Input::from(item.serial.as_str());
                                        app.mode = Mode::Filtering;
                                    }
                                    KeyCode::Esc => {
                                        app.mode = Mode::Normal;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    } else {
                        app.mode = Mode::Normal; // No item selected, go back to normal
                    }
                }
                Mode::ThreadView => {
                    if let Event::Key(key) = event {
                        match key.code {
                            KeyCode::Esc => {
                                app.thread_serial = None;
                                app.mode = Mode::Normal;
                            }
                            _ => {} // Ignore other keys for now
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// A helper function to generate the detail text for the currently selected message.
fn update_detail_text(app: &mut App<'_>, config: &Config) {
    if let Some(selected) = app.list_state.selected() {
        if let Some(item) = app.filtered_and_sorted_items.get(selected) {
            let mut header_lines: Vec<Line> = Vec::new();

            header_lines.push(Line::from(vec![Span::styled(
                "--- Header ---",
                Style::default().fg(Color::LightCyan),
            )]));
            header_lines.push(Line::from(vec![
                Span::styled("Stream: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format!("{:?}", app.stream),
                    Style::default().fg(Color::White),
                ),
            ]));
            header_lines.push(Line::from(vec![
                Span::styled("Sender: ", Style::default().fg(Color::Gray)),
                Span::styled(item.sender.clone(), Style::default().fg(Color::Green)),
            ]));
            if !item.receiver.is_empty() {
                header_lines.push(Line::from(vec![
                    Span::styled("Receiver: ", Style::default().fg(Color::Gray)),
                    Span::styled(item.receiver.clone(), Style::default().fg(Color::Red)),
                ]));
            }
            header_lines.push(Line::from(vec![
                Span::styled("Path: ", Style::default().fg(Color::Gray)),
                Span::styled(item.path.clone(), Style::default().fg(Color::Magenta)),
            ]));
            header_lines.push(Line::from(vec![
                Span::styled("Member: ", Style::default().fg(Color::Gray)),
                Span::styled(item.member.clone(), Style::default().fg(Color::Blue)),
            ]));
            header_lines.push(Line::from(vec![
                Span::styled("Is Reply: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    if item.is_reply { "Yes" } else { "No" },
                    Style::default().fg(if item.is_reply {
                        Color::Green
                    } else {
                        Color::Red
                    }),
                ),
            ]));
            if !item.reply_serial.is_empty() {
                header_lines.push(Line::from(vec![
                    Span::styled("Reply Serial: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        item.reply_serial.clone(),
                        Style::default().fg(Color::Yellow),
                    ),
                ]));
            }
            if !item.serial.is_empty() {
                header_lines.push(Line::from(vec![
                    Span::styled("Serial: ", Style::default().fg(Color::Gray)),
                    Span::styled(item.serial.clone(), Style::default().fg(Color::Yellow)),
                ]));
            }
            header_lines.push(Line::from(vec![Span::raw("")])); // Empty line for spacing

            let detail_text = if let Some(message) = &item.message {
                let body = message.body();
                let body_sig = body.signature();

                if body_sig.to_string().is_empty() {
                    Text::from("[No message body]")
                } else {
                    match body.deserialize::<Structure>() {
                        Ok(structure) => ui::format_value(&Value::from(structure), config),
                        Err(_) => match body.deserialize::<Value>() {
                            Ok(value) => ui::format_value(&value, config),
                            Err(e) => Text::from(format!(
                                "Failed to deserialize body.\n\nSignature: {}\nError: {:#?}",
                                body_sig, e
                            )),
                        },
                    }
                }
            } else {
                Text::from("[No message body]")
            };

            // Prepend header to detail_text
            let mut header_text = Text::from(header_lines);
            header_text.extend(detail_text); // Extend appends lines from one Text to another
            app.detail_text = header_text;

            app.detail_scroll = 0;
        }
    }
}
