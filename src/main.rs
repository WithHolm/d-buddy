// Import necessary crates and modules
use anyhow::Result; // For simplified error handling
use arboard::Clipboard; // For clipboard access
use crossterm::event::{Event, EventStream, KeyCode}; // For handling terminal events like key presses
use futures::stream::StreamExt; // For extending stream functionality, used with zbus MessageStream
use ratatui::prelude::*;
use ratatui::text::Line; // For styled text in the UI
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap}; // UI widgets
use std::{
    io::{self, stdout},
    thread::sleep,
    time::Duration,
}; // Standard I/O for terminal operations
use tokio::{fs, sync::mpsc}; // Asynchronous multi-producer, single-consumer channel for message passing
use tui_input::{backend::crossterm as input_backend, Input}; // For handling text input in the TUI
use zbus::{
    fdo::DBusProxy,
    zvariant::{Structure, Value},
    Message, MessageStream,
}; // For D-Bus communication and message handling

// Maximum number of D-Bus messages to retain in memory
const MAX_MESSAGES: usize = 1000;

// Enum to define the current operating mode of the application
enum Mode {
    Normal,    // Default mode for browsing D-Bus messages
    Filtering, // Mode for entering a filter string
}

// Main application struct holding all the state
struct App {
    messages: Vec<Message>, // List of captured D-Bus messages
    list_state: ListState,  // State of the message list widget (e.g., selected item)
    show_details: bool,     // Flag to indicate if message details popup should be shown
    mode: Mode,             // Current operating mode (Normal or Filtering)
    input: Input,           // Input buffer for the filtering text
    detail_text: String,    // The formatted string for the currently viewed detail
    status_message: String, // A temporary message to show in the status bar
}

// Default implementation for the App struct
impl Default for App {
    fn default() -> Self {
        App {
            messages: Vec::new(),             // Initialize with an empty list of messages
            list_state: ListState::default(), // Default list state (no item selected)
            show_details: false,              // Details popup is hidden by default
            mode: Mode::Normal,               // Start in Normal mode
            input: Input::default(),          // Empty input buffer
            detail_text: String::new(),       // No detail text initially
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
    let (tx, rx) = mpsc::channel(100);
    // Connect to the D-Bus session bus
    let conn = zbus::Connection::session().await?;
    // Create a proxy for the D-Bus server itself.
    let proxy = DBusProxy::new(&conn).await?;
    // Add a match rule to subscribe to all signals on the bus.
    // This is a more robust way to ensure we receive broadcast signals.
    proxy
        .add_match_rule(
            zbus::MatchRule::builder()
                .msg_type(zbus::message::Type::Signal)
                .build(),
        )
        .await?;
    // Create a stream of D-Bus messages from the connection.
    let stream = MessageStream::from(&conn);

    // Spawn a background task to continuously receive D-Bus messages
    tokio::spawn(async move {
        let mut stream = stream;
        // Iterate over the message stream and send each message to the main application via the channel
        while let Some(Ok(msg)) = stream.next().await {
            if tx.send(msg).await.is_err() {
                // If sending fails (e.g., receiver dropped), break the loop
                break;
            }
        }
    });

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
        terminal.draw(|f| ui(f, &mut app))?;

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
                            // Clear status message on any key press in this mode
                            if !app.status_message.is_empty() {
                                app.status_message.clear();
                            }

                            // If details popup is shown, handle its closing and copying
                            if app.show_details {
                                match key.code {
                                    KeyCode::Char('s') | KeyCode::Char(' ') | KeyCode::Esc => {
                                        app.show_details = false;
                                    }
                                    KeyCode::Char('c') => {
                                        let clipboard_status: String;
                                        let file_write_status: String; // Declare here

                                        let text_to_copy = app.detail_text.clone();

                                        // Write to file (this is an async operation, so it must be awaited)
                                        let file_path = "/tmp/d-buddy-details.txt";
                                        file_write_status = match fs::write(file_path, text_to_copy.as_bytes()).await {
                                            Ok(_) => format!("Saved to {}", file_path),
                                            Err(e) => format!("Failed to save to file: {}", e),
                                        };

                                        clipboard_status = match clipboard.set_text(text_to_copy.clone()) {
                                            Ok(_) => {
                                                // This short sleep is a workaround for Linux clipboard ownership.
                                                sleep(Duration::from_millis(100));
                                                "Copied to clipboard!".to_string()
                                            },
                                            Err(e) => format!("Copy failed: {}", e),
                                        };

                                        // if let Some(cb) = &mut clipboard {
                                        //     match cb.set_text(text_to_copy.clone()) { // Use text_to_copy
                                        //         Ok(_) => {
                                        //             // This short sleep is a workaround for Linux clipboard ownership.
                                        //             sleep(Duration::from_millis(100));
                                        //             clipboard_status = "Copied to clipboard!".to_string();
                                        //         }
                                        //         Err(e) => {
                                        //             clipboard_status = format!("Copy failed: {}", e);
                                        //         }
                                        //     }
                                        // } else {
                                        //     clipboard_status = "Clipboard not available.".to_string();
                                        // }

                                        app.status_message = format!("{} | {}", file_write_status, clipboard_status);
                                    }
                                    _ => {} // Ignore other keys
                                }
                            } else {
                                // Handle key presses in Normal mode (when details are hidden)
                                match key.code {
                                    KeyCode::Char('q') => break, // Quit the application
                                    KeyCode::Down => {
                                        // Move selection down, guarding against an empty list
                                        if !app.messages.is_empty() {
                                            let i = match app.list_state.selected() {
                                                Some(i) => (i + 1).min(app.messages.len() - 1),
                                                None => 0,
                                            };
                                            app.list_state.select(Some(i));
                                        }
                                    }
                                    KeyCode::Up => {
                                        // Move selection up
                                        if !app.messages.is_empty() {
                                            let i = match app.list_state.selected() {
                                                Some(i) => i.saturating_sub(1),
                                                None => 0,
                                            };
                                            app.list_state.select(Some(i));
                                        }
                                    }
                                    KeyCode::Char('s') | KeyCode::Char(' ') => {
                                        // Generate detail text and show the popup
                                        if let Some(selected) = app.list_state.selected() {
                                            if let Some(message) = app.messages.get(selected) {
                                                let body = message.body();
                                                let body_sig = body.signature();

                                                app.detail_text = if body_sig.to_string().is_empty() {
                                                    "[No message body]".to_string()
                                                } else {
                                                    // Attempt to deserialize as a `Structure` first, as this is a common case.
                                                    match body.deserialize::<Structure>() {
                                                        Ok(structure) => format_value(&Value::from(structure)),
                                                        Err(_) => {
                                                            // If that fails, fall back to deserializing as a generic `Value`.
                                                            match body.deserialize::<Value>() {
                                                                Ok(value) => format_value(&value),
                                                                Err(e) => format!(
                                                                    "Failed to deserialize body.\n\nSignature: {}\nError: {:#?}",
                                                                    body_sig,
                                                                    e
                                                                ),
                                                            }
                                                        }
                                                    }
                                                };
                                                app.show_details = true;
                                            }
                                        }
                                    }
                                    KeyCode::Char('/') => {
                                        // Enter Filtering mode
                                        app.mode = Mode::Filtering;
                                    }
                                    _ => {} // Ignore other keys
                                }
                            }
                        }
                    }
                    Mode::Filtering => {
                        if let Event::Key(key) = event {
                            // Handle key presses in Filtering mode
                            match key.code {
                                KeyCode::Enter => {
                                    app.mode = Mode::Normal; // Exit Filtering mode
                                }
                                KeyCode::Esc => {
                                    app.input.reset(); // Clear filter input
                                    app.mode = Mode::Normal; // Exit Filtering mode
                                }
                                _ => {
                                    // Pass other key events to the input widget
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

// Draws the application's user interface
fn ui(frame: &mut Frame, app: &mut App) {
    // Define the main layout with two chunks: one for the message list, one for the input/status
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
        .split(frame.area());

    // Define a layout for the main message display area
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(100)].as_ref())
        .split(chunks[0]);

    let filter_text = app.input.value(); // Get the current filter text from the input widget
                                         // Prepare ListItems from D-Bus messages, filtering them if a filter is active
    let items: Vec<ListItem> = app
        .messages
        .iter()
        .filter(|msg| {
            // Get D-Bus message header details
            let header = msg.header();
            let sender = header.sender().map(|s| s.as_str()).unwrap_or("");
            let member = header.member().map(|s| s.as_str()).unwrap_or("");
            let path = header.path().map(|p| p.as_str()).unwrap_or("");
            // Check if any of the header fields contain the filter text
            sender.contains(filter_text)
                || member.contains(filter_text)
                || path.contains(filter_text)
        })
        .map(|msg| {
            // Format each message into a displayable string
            let header = msg.header();
            let text = format!(
                "sender: {}, member: {}, path:જી {}",
                header.sender().map(|s| s.as_str()).unwrap_or(""),
                header.member().map(|s| s.as_str()).unwrap_or(""),
                header.path().map(|p| p.as_str()).unwrap_or(""),
            );
            ListItem::new(text) // Create a ListItem
        })
        .collect();

    // Create the List widget for displaying D-Bus messages
    let list = List::new(items)
        .block(
            Block::default()
                .title("D-Bus Signals") // Set title for the block
                .borders(Borders::ALL),
        )
        .highlight_symbol("> "); // Symbol to indicate selected item

    // Render the message list widget
    frame.render_stateful_widget(list, main_chunks[0], &mut app.list_state);

    // If show_details is true, render the message details popup
    if app.show_details {
        // Create a block for the details popup
        let block = Block::default()
            .title("Message Details")
            .borders(Borders::ALL);
        // Calculate the centered area for the popup
        let area = centered_rect(60, 60, frame.area());
        // Create a Paragraph widget with the pre-formatted detail text
        let paragraph = Paragraph::new(app.detail_text.clone())
            .block(block)
            .wrap(Wrap { trim: false }); // Allow text wrapping
        frame.render_widget(Clear, area); // Clear the background behind the popup
        frame.render_widget(paragraph, area); // Render the details paragraph
    }

    // In the bottom chunk, render either the filter input box or the keybindings help text
    match app.mode {
        Mode::Filtering => {
            // Calculate width for the input box and scrolling
            let width = chunks[1].width.max(3) - 3;
            let scroll = app.input.visual_scroll(width as usize);
            // Create a Paragraph widget for the input text
            let input = Paragraph::new(app.input.value())
                .scroll((0, scroll as u16)) // Handle scrolling of input text
                .block(Block::default().borders(Borders::ALL).title("Filter")); // Add border and title
            frame.render_widget(input, chunks[1]); // Render the input box in the bottom chunk
        }
        Mode::Normal => {
            let help_text = if !app.status_message.is_empty() {
                Line::from(app.status_message.as_str().yellow())
            } else if app.show_details {
                Line::from(vec![
                    "c".bold(),
                    ": copy | ".into(),
                    "s".bold(),
                    "/".dim(),
                    "space".bold(),
                    "/".dim(),
                    "esc".bold(),
                    ": close".into(),
                ])
            } else {
                Line::from(vec![
                    "q".bold(),
                    ": quit | ".into(),
                    "/".bold(),
                    ": filter | ".into(),
                    "s".bold(),
                    "/".dim(),
                    "space".bold(),
                    ": details | ".into(),
                    "↑".bold(),
                    "/".dim(),
                    "↓".bold(),
                    ": navigate".into(),
                ])
            };
            let help_paragraph = Paragraph::new(help_text)
                .block(Block::default().borders(Borders::ALL).title("Keys"));
            frame.render_widget(help_paragraph, chunks[1]);
        }
    }
}

// Helper function to format a `zbus::zvariant::Value` in a YAML-like, readable way.
fn format_value(value: &Value) -> String {
    // Inner recursive function to handle nesting and indentation.
    fn format_recursive(value: &Value, indent: usize, prefix: &str) -> String {
        // Handle variants by unwrapping them and formatting the inner value directly.
        if let Value::Value(inner) = value {
            return format_recursive(inner, indent, prefix);
        }

        let indent_str = "  ".repeat(indent);

        // Helper to get a string representation of a Value's type
        fn get_value_type_str(value: &Value) -> &'static str {
            match value {
                Value::U8(_) => "u8",
                Value::I16(_) => "i16",
                Value::U16(_) => "u16",
                Value::I32(_) => "i32",
                Value::U32(_) => "u32",
                Value::I64(_) => "i64",
                Value::U64(_) => "u64",
                Value::F64(_) => "f64",
                Value::Bool(_) => "bool",
                Value::Str(_) => "str",
                Value::Signature(_) => "signature",
                Value::ObjectPath(_) => "object-path",
                Value::Array(_) => "array",
                Value::Structure(_) => "struct",
                Value::Dict(_) => "dict",
                Value::Value(_) => "variant",
                Value::Fd(_) => "fd",
            }
        }

        // Handle simple, single-line values first.
        match value {
            Value::U8(v) => return format!("{}{} [u8]: {}", indent_str, prefix, v),
            Value::I16(v) => return format!("{}{} [i16]: {}", indent_str, prefix, v),
            Value::U16(v) => return format!("{}{} [u16]: {}", indent_str, prefix, v),
            Value::I32(v) => return format!("{}{} [i32]: {}", indent_str, prefix, v),
            Value::U32(v) => return format!("{}{} [u32]: {}", indent_str, prefix, v),
            Value::I64(v) => return format!("{}{} [i64]: {}", indent_str, prefix, v),
            Value::U64(v) => return format!("{}{} [u64]: {}", indent_str, prefix, v),
            Value::F64(v) => return format!("{}{} [f64]: {}", indent_str, prefix, v),
            Value::Bool(v) => return format!("{}{} [bool]: {}", indent_str, prefix, v),
            Value::Str(s) => return format!("{}{} [str]: \"{}\"", indent_str, prefix, s),
            Value::Signature(s) => return format!("{}{} [signature]: '{}'", indent_str, prefix, s),
            Value::ObjectPath(p) => {
                return format!("{}{} [object-path]: {}", indent_str, prefix, p.as_str())
            }
            Value::Fd(f) => return format!("{}{} [fd]: {:?}", indent_str, prefix, f),
            // This case is now reachable for `Value::Value`
            _ => (), // Continue to complex types
        }

        // Handle complex, multi-line values.
        let mut output = format!(
            "{}{} [{}]:\n",
            indent_str,
            prefix,
            get_value_type_str(value)
        );

        match value {
            Value::Array(arr) => {
                if arr.is_empty() {
                    return format!("{}{} [array]: (empty)", indent_str, prefix);
                }

                // Special Case 1: Array of `(String, Value)` structs, to be displayed like a dict.
                let is_kv_struct_array = arr.iter().all(|item| {
                    if let Value::Structure(s) = item {
                        if s.fields().len() == 2 {
                            if let Value::Str(_) = &s.fields()[0] {
                                return true;
                            }
                        }
                    }
                    false
                });

                if is_kv_struct_array {
                    let mut new_output = format!("{}{} [struct[]]:\n", indent_str, prefix);
                    let key_indent_str = "  ".repeat(indent + 1);

                    for item in arr.iter() {
                        if let Value::Structure(s) = item {
                            if let (Value::Str(key), value) = (&s.fields()[0], &s.fields()[1]) {
                                // Print the key on its own indented line.
                                new_output.push_str(&format!("{}{}:\n", key_indent_str, key));
                                // Format the value on subsequent lines, further indented.
                                let value_str = format_recursive(value, indent + 2, "");
                                new_output.push_str(&value_str);
                                new_output.push('\n');
                            }
                        }
                    }
                    return new_output.trim_end().to_string();
                }

                // Special Case 2: Homogenous array of simple types.
                let first_val = &arr[0];
                let is_simple_type =
                    !matches!(first_val, Value::Array(_) | Value::Structure(_) | Value::Dict(_));

                if is_simple_type {
                    let first_type_str = get_value_type_str(first_val);
                    let all_same_simple_type = arr
                        .iter()
                        .skip(1)
                        .all(|v| get_value_type_str(v) == first_type_str);

                    if all_same_simple_type {
                        let mut new_output =
                            format!("{}{} [{}[]]:\n", indent_str, prefix, first_type_str);
                        let item_indent_str = "  ".repeat(indent + 1);

                        if first_type_str == "u8" {
                            for item in arr.iter() {
                                if let Value::U8(b) = item {
                                    new_output.push_str(&format!(
                                        "{}byte 0x{:02x}\n",
                                        item_indent_str, b
                                    ));
                                }
                            }
                        } else {
                            for item in arr.iter() {
                                let value_content = match item {
                                    Value::Str(s) => format!("\"{}\"", s),
                                    Value::Signature(s) => format!("'{}'", s),
                                    _ => item.to_string(),
                                };
                                new_output
                                    .push_str(&format!("{}{}\n", item_indent_str, value_content));
                            }
                        }
                        return new_output.trim_end().to_string();
                    }
                }

                // Fallback for all other array types.
                output = format!("{}{} [array]:\n", indent_str, prefix);
                for (i, item) in arr.iter().enumerate() {
                    output.push_str(&format_recursive(item, indent + 1, &i.to_string()));
                    output.push('\n');
                }
            }
            Value::Structure(s) => {
                if s.fields().is_empty() {
                    return format!("{}{} [struct]: (empty)", indent_str, prefix);
                }
                for (i, field) in s.fields().iter().enumerate() {
                    output.push_str(&format_recursive(
                        field,
                        indent + 1,
                        &format!("i_{}", i + 1),
                    ));
                    output.push('\n');
                }
            }
            Value::Dict(d) => {
                if d.iter().count() == 0 {
                    return format!("{}{} [dict]: {{}}", indent_str, prefix);
                }

                let mut entries: Vec<(String, &Value)> = Vec::new();
                let mut max_key_len = 0;

                // First pass: collect key strings and find the maximum key length for alignment.
                for (k, v) in d.iter() {
                    let key_str = match k {
                        Value::Str(s) => s.to_string(),
                        _ => format!("{:?}", k).trim_matches('"').to_string(),
                    };
                    max_key_len = max_key_len.max(key_str.len());
                    entries.push((key_str, v));
                }

                let mut dict_output_lines = Vec::new();
                let inner_indent_for_value = indent + 1; // Indent level for the dictionary items
                let inner_indent_str = "  ".repeat(inner_indent_for_value);

                // Second pass: format each entry with alignment.
                for (key_str, v) in entries {
                    let padding_for_key = " ".repeat(max_key_len.saturating_sub(key_str.len()));

                    // format_recursive(v, 0, "") will produce "[type]: value" or multi-line complex structure
                    // starting without initial indent or prefix.
                    let formatted_value_segment = format_recursive(v, 0, "");

                    let lines: Vec<&str> = formatted_value_segment.lines().collect();

                    if lines.is_empty() {
                        dict_output_lines.push(format!("{}{} {}:", inner_indent_str, key_str, padding_for_key));
                    } else {
                        dict_output_lines.push(format!("{}{} {}: {}", inner_indent_str, key_str, padding_for_key, lines[0]));

                        // Calculate the column where the value starts for subsequent lines.
                        // This is current indent + key_str_len + padding + ": " (which is 2 characters)
                        let value_start_col = (inner_indent_for_value * 2) + max_key_len + 2;
                        for &line in lines.iter().skip(1) {
                            dict_output_lines.push(format!("{}{}", " ".repeat(value_start_col), line));
                        }
                    }
                }
                output.push_str(&dict_output_lines.join("\n"));
            }
            // This case will now not be hit for Value::Value, but is kept for other complex types.
            _ => {
                output.push_str(&format!("{}{:?}", "  ".repeat(indent + 1), value));
                output.push('\n');
            }
        }

        output.trim_end().to_string()
    }

    // Special handling for top-level `Structure` to match desired output format.
    if let Value::Structure(s) = value {
        let mut output = String::new();
        for (i, field) in s.fields().iter().enumerate() {
            output.push_str(&format_recursive(field, 0, &format!("i_{}", i + 1)));
            output.push('\n');
        }
        return output.trim_end().to_string();
    }

    // Fallback for any non-Structure top-level value.
    format_recursive(value, 0, "value")
}

/// Helper function to create a centered rectangle given a percentage of the available area.
/// This is typically used for popups or modal dialogs.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    // Create a vertical layout to center the rectangle vertically
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2), // Top padding
            Constraint::Percentage(percent_y),             // Content area
            Constraint::Percentage((100 - percent_y) / 2), // Bottom padding
        ])
        .split(r);

    // Create a horizontal layout to center the content area horizontally
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2), // Left padding
            Constraint::Percentage(percent_x),             // Content area
            Constraint::Percentage((100 - percent_x) / 2), // Right padding
        ])
        .split(popup_layout[1])[1] // Get the central content area
}
