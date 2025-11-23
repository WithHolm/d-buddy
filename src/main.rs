mod bus;
mod ui;

// Import necessary crates and modules
use anyhow::Result; // For simplified error handling
use arboard::Clipboard; // For clipboard access
use crossterm::event::{Event, EventStream, KeyCode}; // For handling terminal events like key presses
use futures::stream::StreamExt; // For extending stream functionality, used with zbus MessageStream
use ratatui::prelude::*;
use ratatui::widgets::ListState;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
// UI widgets
use bus::Item;
use std::{
    io::{self, stdout},
    thread::sleep,
    time::Duration,
}; // Standard I/O for terminal operations
use tokio::{fs, sync::mpsc}; // Asynchronous multi-producer, single-consumer channel for message passing
use tui_input::{backend::crossterm as input_backend, Input}; // For handling text input in the TUI
use zbus::{
    zvariant::{Structure, Value},
    Message,
}; // For D-Bus communication and message handling

// Maximum number of D-Bus messages to retain in memory
const MAX_MESSAGES: usize = 1000;

// Enum to define the current operating mode of the application
enum Mode {
    Normal,    // Default mode for browsing D-Bus messages
    Filtering, // Mode for entering a filter string
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum BusType {
    Session = 0,
    System = 1,
    Both = 2,
}

// Main application struct holding all the state
struct App {
    stream: BusType,
    messages: HashMap<BusType, Arc<tokio::sync::Mutex<Vec<bus::Item>>>>,
    list_state: ListState, // State of the message list widget (e.g., selected item)
    show_details: bool,    // Flag to indicate if message details popup should be shown
    mode: Mode,            // Current operating mode (Normal or Filtering)
    input: Input,          // Input buffer for the filtering text
    detail_text: String,   // The formatted string for the currently viewed detail
    detail_scroll: u16,    // The vertical scroll offset for the detail view
    status_message: String, // A temporary message to show in the status bar
}

fn new_messages_hashmap() -> HashMap<BusType, Arc<tokio::sync::Mutex<Vec<bus::Item>>>> {
    let mut map = HashMap::new();

    for bus_type in [BusType::Session, BusType::System] {
        map.insert(bus_type, Arc::new(tokio::sync::Mutex::new(Vec::new())));
    }

    map
}

// Default implementation for the App struct
impl Default for App {
    fn default() -> Self {
        App {
            stream: BusType::Session,
            messages: new_messages_hashmap(), // Initialize with an empty list of messages
            list_state: ListState::default(), // Default list state (no item selected)
            show_details: false,              // Details popup is hidden by default
            mode: Mode::Normal,               // Start in Normal mode
            input: Input::default(),          // Empty input buffer
            detail_text: String::new(),       // No detail text initially
            detail_scroll: 0,                 // Start with no scroll
            status_message: String::new(),    // No status message initially
        }
    }
}

// Main asynchronous entry point of the application
#[tokio::main]
async fn main() -> Result<()> {
    // Setup the terminal for TUI (raw mode, alternate screen)
    let mut terminal = setup_terminal()?;
    // Create an asynchronous channel to send D-Bus messages from the background task to the main loop
    // let (tx, rx) = mpsc::channel(100);
    // Connect to the D-Bus session bus
    // let conn = zbus::Connection::session().await?;
    // // Create a proxy for the D-Bus server itself.
    // let proxy = DBusProxy::new(&conn).await?;
    // Add a match rule to subscribe to all signals on the bus.
    // This is a more robust way to ensure we receive broadcast signals.
    // proxy
    //     .add_match_rule(
    //         zbus::MatchRule::builder()
    //             .msg_type(zbus::message::Type::Signal)
    //             .build(),
    //     )
    //     .await?;
    // // Create a stream of D-Bus messages from the connection.
    // let stream = MessageStream::from(&conn);

    // // Spawn a background task to continuously receive D-Bus messages
    // tokio::spawn(async move {
    //     let mut stream = stream;
    //     // Iterate over the message stream and send each message to the main application via the channel
    //     while let Some(Ok(msg)) = stream.next().await {
    //         if tx.send(msg).await.is_err() {
    //             // If sending fails (e.g., receiver dropped), break the loop
    //             break;
    //         }
    //     }
    // });
    //

    // Run the main application loop
    run(&mut terminal, rx).await?;
    // Restore the terminal to its original state before exiting
    restore_terminal()?;
    Ok(())
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
async fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut rx: mpsc::Receiver<Message>,
) -> Result<()> {
    let mut app = App::default(); // Initialize the application state
    let mut event_stream = EventStream::new(); // Create a stream of terminal events
    let mut clipboard = Clipboard::new().unwrap();

    loop {
        // Draw the UI, then wait for the next event.
        terminal.draw(|f| ui::ui(f, &mut app))?;

        // Use tokio::select! to wait for either a D-Bus message or a terminal event.
        tokio::select! {
            // Branch for receiving a D-Bus message.
            Some(msg) = rx.recv() => {
                if app.messages.len() == MAX_MESSAGES {
                    app.messages.remove(0);
                }
                app.messages.push(msg);
                // If no message is selected, select the newly added one.
                if app.list_state.selected().is_none() {
                    app.list_state.select(Some(app.messages.len() - 1));
                }
            }

            // Branch for receiving a terminal input event.
            Some(Ok(event)) = event_stream.next() => {
                 match app.mode {
                    Mode::Normal => {
                        if let Event::Key(key) = event {
                            if !app.status_message.is_empty() {
                                app.status_message.clear();
                            }
                            match key.code {
                                KeyCode::Char('q') => {
                                    if !app.show_details {
                                        break
                                    }
                                    app.show_details = false;
                                }
                                KeyCode::Char('/') => app.mode = Mode::Filtering,
                                KeyCode::Up => {
                                    if !app.messages.is_empty() {
                                        let i = match app.list_state.selected() {
                                            Some(i) => i.saturating_sub(1),
                                            None => 0,
                                        };
                                        app.list_state.select(Some(i));
                                        if app.show_details {
                                            update_detail_text(&mut app);
                                        }
                                    }
                                }
                                KeyCode::Down => {
                                    if !app.messages.is_empty() {
                                        let i = match app.list_state.selected() {
                                            Some(i) => (i + 1).min(app.messages.len() - 1),
                                            None => 0,
                                        };
                                        app.list_state.select(Some(i));
                                        if app.show_details {
                                            update_detail_text(&mut app);
                                        }
                                    }
                                }
                                KeyCode::Char('s') | KeyCode::Char(' ') => {
                                    if app.show_details {
                                        app.show_details = false;
                                    } else {
                                        update_detail_text(&mut app);
                                        app.show_details = true;
                                    }
                                }
                                KeyCode::Esc => {
                                    if app.show_details {
                                        app.show_details = false;
                                    }
                                }
                                KeyCode::Char('c') => {
                                    if app.show_details {
                                        let text_to_copy = app.detail_text.clone();
                                        let file_path = "/tmp/d-buddy-details.txt";
                                        let file_write_status = match fs::write(file_path, text_to_copy.as_bytes()).await {
                                            Ok(_) => format!("Saved to {}", file_path),
                                            Err(e) => format!("Failed to save to file: {}", e),
                                        };
                                        let clipboard_status = match clipboard.set_text(text_to_copy) {
                                            Ok(_) => {
                                                sleep(Duration::from_millis(100));
                                                "Copied to clipboard!".to_string()
                                            },
                                            Err(e) => format!("Copy failed: {}", e),
                                        };
                                        app.status_message = format!("{} | {}", file_write_status, clipboard_status);
                                    }
                                }
                                KeyCode::Char('j') => {
                                    if app.show_details {
                                        app.detail_scroll = app.detail_scroll.saturating_add(1);
                                    }
                                }
                                KeyCode::Char('k') => {
                                    if app.show_details {
                                        app.detail_scroll = app.detail_scroll.saturating_sub(1);
                                    }
                                }
                                KeyCode::PageDown => {
                                    if app.show_details {
                                        app.detail_scroll = app.detail_scroll.saturating_add(10);
                                    }
                                }
                                KeyCode::PageUp => {
                                    if app.show_details {
                                        app.detail_scroll = app.detail_scroll.saturating_sub(10);
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
                                    app.mode = Mode::Normal;
                                }
                                KeyCode::Esc => {
                                    app.input.reset();
                                    app.mode = Mode::Normal;
                                }
                                _ => {
                                    input_backend::to_input_request(&event)
                                        .and_then(|req| app.input.handle(req));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// A helper function to generate the detail text for the currently selected message.
fn update_detail_text(app: &mut App) {
    if let Some(selected) = app.list_state.selected() {
        if let Some(message) = app.messages.get(selected) {
            let body = message.body();
            let body_sig = body.signature();

            app.detail_text = if body_sig.to_string().is_empty() {
                "[No message body]".to_string()
            } else {
                match body.deserialize::<Structure>() {
                    Ok(structure) => ui::format_value(&Value::from(structure)),
                    Err(_) => match body.deserialize::<Value>() {
                        Ok(value) => ui::format_value(&value),
                        Err(e) => format!(
                            "Failed to deserialize body.\n\nSignature: {}\nError: {:#?}",
                            body_sig, e
                        ),
                    },
                }
            };
            app.detail_scroll = 0;
        }
    }
}
