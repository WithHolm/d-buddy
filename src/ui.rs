use super::{App, Mode};
use ratatui::{
    prelude::*,
    text::Line,
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use zbus::zvariant::{Structure, Value};

// Draws the application's user interface
pub fn ui(frame: &mut Frame, app: &mut App) {
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
            // let now = SystemTime::now().;
            // let secs = now.as_secs();
            // let millis = now.subsec_millis();
            // let timestamp = format!("{}.{:03}", secs, millis);
            let timestamp = "mm.ss.000";
            let text = format!(
                "[{}] sender: {}, member: {}, path:જી {}",
                timestamp,
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
        let area = centered_rect(80, 80, frame.area());
        let popup_inner_height = area.height.saturating_sub(2);

        // Check if scrolling is possible
        let num_text_lines = app.detail_text.lines().count() as u16;
        let can_scroll_up = app.detail_scroll > 0;
        let can_scroll_down = app.detail_scroll + popup_inner_height < num_text_lines;

        // Create a dynamic title with scroll indicators
        let title = match (can_scroll_up, can_scroll_down) {
            (true, true) => "Message Details [↑...↓]",
            (true, false) => "Message Details [↑...]",
            (false, true) => "Message Details [...↓]",
            (false, false) => "Message Details",
        };
        let block = Block::default().title(title).borders(Borders::ALL);

        // Create a Paragraph widget with the pre-formatted detail text and scroll state
        let paragraph = Paragraph::new(app.detail_text.clone())
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((app.detail_scroll, 0));

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
                    "esc".bold(),
                    ": close | ".into(),
                    "j".bold(),
                    "/".dim(),
                    "k".bold(),
                    "/".dim(),
                    "PgUp".bold(),
                    "/".dim(),
                    "PgDn".bold(),
                    ": scroll".into(),
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
pub fn format_value(value: &Value) -> String {
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
                let is_simple_type = !matches!(
                    first_val,
                    Value::Array(_) | Value::Structure(_) | Value::Dict(_)
                );

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
                        dict_output_lines.push(format!(
                            "{}{} {}:",
                            inner_indent_str, key_str, padding_for_key
                        ));
                    } else {
                        dict_output_lines.push(format!(
                            "{}{} {}: {}",
                            inner_indent_str, key_str, padding_for_key, lines[0]
                        ));

                        // Calculate the column where the value starts for subsequent lines.
                        // This is current indent + key_str_len + padding + ": " (which is 2 characters)
                        let value_start_col = (inner_indent_for_value * 2) + max_key_len + 2;
                        for &line in lines.iter().skip(1) {
                            dict_output_lines.push(format!(
                                "{}{}",
                                " ".repeat(value_start_col),
                                line
                            ));
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
