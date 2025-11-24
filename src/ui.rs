use super::{App, Config, Mode};
use crate::bus::BusType;
use ratatui::{
    prelude::*,
    text::{Line, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};
use zbus::zvariant::Value;

// Draws the application's user interface
pub fn ui<'a>(frame: &mut Frame, app: &mut App<'a>, config: &Config) {
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

    let items = app.list_items.clone();

    // Create the List widget for displaying D-Bus messages
    let (session_style, system_style, both_style) = match app.stream {
        BusType::Session => (
            Style::default().fg(config.color_stream_session).bold(),
            Style::default().fg(config.color_keybind_text).italic(),
            Style::default().fg(config.color_keybind_text).italic(),
        ),
        BusType::System => (
            Style::default().fg(config.color_keybind_text).italic(),
            Style::default().fg(config.color_stream_system).bold(),
            Style::default().fg(config.color_keybind_text).italic(),
        ),
        BusType::Both => (
            Style::default().fg(config.color_keybind_text).italic(),
            Style::default().fg(config.color_keybind_text).italic(),
            Style::default().fg(config.color_stream_session).bold(), // Reusing session color for "Both" active
        ),
    };

    let title_spans = Line::from(vec![
        Span::raw("D-Bus Signals ["),
        Span::styled("Session", session_style),
        Span::raw("|"),
        Span::styled("System", system_style),
        Span::raw("|"),
        Span::styled("Both", both_style),
        Span::raw("]"),
    ]);

    let list = List::new(items)
        .block(
            Block::default()
                .title(title_spans) // Set title for the block
                .borders(Borders::ALL),
        )
        .highlight_symbol(if app.list_state.selected().is_some() {
            "> "
        } else {
            ""
        }); // Symbol to indicate selected item

    // Render the message list widget
    frame.render_stateful_widget(list, main_chunks[0], &mut app.list_state);

    // Render Filtering popup as an overlay if in Mode::Filtering
    if let Mode::Filtering = app.mode {
        let area = centered_rect(80, 7, frame.area()); // Use frame.size() for full screen relative centering
        let block = Block::default().title("Filter").borders(Borders::ALL);
        frame.render_widget(Clear, area);
        frame.render_widget(&block, area);

        let inner_area = block.inner(area);

        // Calculate width for the input box and scrolling within the popup
        let width = inner_area.width.max(3) - 3;
        let scroll = app.input.visual_scroll(width as usize);

        let mut filter_display_text = String::new();
        if !app.filter_criteria.is_empty() {
            let criteria_vec: Vec<String> = app
                .filter_criteria
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
            filter_display_text.push_str(&format!("[{}] ", criteria_vec.join(", ")));
        }
        filter_display_text.push_str(app.input.value());

        // Create a Paragraph widget for the input text, now always single line
        let input = Paragraph::new(filter_display_text).scroll((0, scroll as u16));
        frame.render_widget(&input, inner_area);
    }

    // Render Filtering popup as an overlay if in Mode::Filtering
    if let Mode::Filtering = app.mode {
        let area = centered_rect(80, 7, frame.area()); // Use frame.size() for full screen relative centering
        let block = Block::default().title("Filter").borders(Borders::ALL);
        frame.render_widget(Clear, area);
        frame.render_widget(&block, area);

        let inner_area = block.inner(area);

        // Calculate width for the input box and scrolling within the popup
        let width = inner_area.width.max(3) - 3;
        let scroll = app.input.visual_scroll(width as usize);

        let mut filter_display_text = String::new();
        if !app.filter_criteria.is_empty() {
            let criteria_vec: Vec<String> = app
                .filter_criteria
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
            filter_display_text.push_str(&format!("[{}] ", criteria_vec.join(", ")));
        }
        filter_display_text.push_str(app.input.value());

        // Create a Paragraph widget for the input text, now always single line
        let input = Paragraph::new(filter_display_text).scroll((0, scroll as u16));
        frame.render_widget(&input, inner_area);
    }

    // Render AutoFilterSelection popup as an overlay if in Mode::AutoFilterSelection
    if let Mode::AutoFilterSelection = app.mode {
        let area = centered_rect(60, 30, frame.area());
        let block = Block::default()
            .title("Select AutoFilter Field")
            .borders(Borders::ALL);
        frame.render_widget(Clear, area);
        frame.render_widget(&block, area);

        let inner_area = block.inner(area);

        let autofilter_options = ["sender", "member", "path", "serial", "reply_serial"];
        let mut list_items: Vec<ListItem> = Vec::new();

        if let Some(selected_index) = app.list_state.selected() {
            if let Some(item) = app.filtered_and_sorted_items.get(selected_index) {
                for &option in autofilter_options.iter() {
                    let example_value = match option {
                        "sender" => item.sender.as_str(),
                        "member" => item.member.as_str(),
                        "path" => item.path.as_str(),
                        "serial" => item.serial.as_str(),
                        "reply_serial" => item.reply_serial.as_str(),
                        _ => "",
                    };
                    list_items.push(ListItem::new(Line::from(vec![
                        Span::raw(format!("{}: ", option)),
                        Span::styled(
                            example_value,
                            Style::default().fg(config.color_autofilter_value),
                        ),
                    ])));
                }
            } else { /* ... */
            }
        } else { /* ... */
        }

        let list = List::new(list_items)
            .block(Block::default())
            .highlight_symbol("> ")
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(config.color_selection_highlight_fg)
                    .bg(config.color_selection_highlight_bg),
            );

        frame.render_stateful_widget(list, inner_area, &mut app.autofilter_selection_state);
    }

    // If show_details is true, render the message details popup
    if app.show_details {
        let area = centered_rect(80, 80, frame.area());
        let popup_inner_height = area.height.saturating_sub(2);

        let num_text_lines = app.detail_text.lines.len() as u16;
        let max_scroll = num_text_lines.saturating_sub(popup_inner_height);

        if let Some(delta) = app.detail_scroll_request.take() {
            app.detail_scroll = (app.detail_scroll as i32 + delta).max(0) as u16;
        }
        app.detail_scroll = app.detail_scroll.min(max_scroll);

        // Check if scrolling is possible
        let can_scroll_up = app.detail_scroll > 0;
        let can_scroll_down = app.detail_scroll + popup_inner_height < num_text_lines;
        // Create a dynamic title with scroll indicators
        let scroll_indicator = match (can_scroll_up, can_scroll_down) {
            (true, true) => "[↑...↓]",
            (true, false) => "[↑...]",
            (false, true) => "[...↓]",
            (false, false) => "",
        };

        let title_spans = Line::from(vec![
            Span::raw("Message Details "),
            Span::raw(scroll_indicator),
        ]);
        let block = Block::default().title(title_spans).borders(Borders::ALL);

        // Create a Paragraph widget with the pre-formatted detail text and scroll state
        let paragraph = Paragraph::new(app.detail_text.clone())
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((app.detail_scroll, 0));

        frame.render_widget(Clear, area); // Clear the background behind the popup
        frame.render_widget(paragraph, area); // Render the details paragraph
    }

    // Render the appropriate keybinds/status in the bottom chunk based on the current mode
    match app.mode {
        Mode::Filtering => {
            let key_hints = Line::from(vec![
                "Esc".bold().fg(config.color_keybind_key),
                ": clear | ".into(),
                "Enter".bold().fg(config.color_keybind_key),
                ": apply | ".into(),
                "Tab".bold().fg(config.color_keybind_key),
                ": autofilter".into(),
            ]);
            let help_paragraph = Paragraph::new(key_hints)
                .block(Block::default().borders(Borders::ALL).title("Keys"));
            frame.render_widget(help_paragraph, chunks[1]);
        }
        Mode::Normal => {
            let help_text = if !app.status_message.is_empty() {
                Line::from(app.status_message.as_str().fg(config.color_status_message))
            } else if app.show_details {
                Line::from(vec![
                    "c".bold().fg(config.color_keybind_key),
                    ": copy | ".into(),
                    "s".bold().fg(config.color_keybind_key),
                    "/".dim(),
                    "esc".bold().fg(config.color_keybind_key),
                    ": close | ".into(),
                    "j".bold().fg(config.color_keybind_key),
                    "/".dim(),
                    "k".bold().fg(config.color_keybind_key),
                    "/".dim(),
                    "PgUp".bold().fg(config.color_keybind_key),
                    "/".dim(),
                    "PgDn".bold().fg(config.color_keybind_key),
                    ": scroll".into(),
                ])
            } else {
                Line::from(vec![
                    "q".bold().fg(config.color_keybind_key),
                    ": quit | ".into(),
                    "Tab".bold().fg(config.color_keybind_key),
                    ": view | ".into(),
                    "f".bold().fg(config.color_keybind_key),
                    ": filter | ".into(),
                    "g".bold().fg(config.color_keybind_key),
                    ": group | ".into(),
                    "r".bold().fg(config.color_keybind_key),
                    ": reply | ".into(),
                    "x".bold().fg(config.color_keybind_key),
                    ": clear | ".into(),
                    "s".bold().fg(config.color_keybind_key),
                    "/".dim(),
                    "space".bold().fg(config.color_keybind_key),
                    ": details | ".into(),
                    "↑".bold().fg(config.color_keybind_key),
                    "/".dim(),
                    "↓".bold().fg(config.color_keybind_key),
                    ": navigate".into(),
                ])
            };
            let help_paragraph = Paragraph::new(help_text)
                .block(Block::default().borders(Borders::ALL).title("Keys"));
            frame.render_widget(help_paragraph, chunks[1]);
        }
        Mode::AutoFilterSelection => {
            let help_paragraph = Paragraph::new(Line::from(vec![
                "Esc".bold().fg(config.color_keybind_key),
                ": cancel | ".into(),
                "Enter".bold().fg(config.color_keybind_key),
                ": select | ".into(),
                "↑".bold().fg(config.color_keybind_key),
                "/".dim(),
                "↓".bold().fg(config.color_keybind_key),
                ": navigate".into(),
            ]))
            .block(Block::default().borders(Borders::ALL).title("Autofilter"));
            frame.render_widget(help_paragraph, chunks[1]);
        }
        Mode::ThreadView => {
            let thread_serial_display = app.thread_serial.as_deref().unwrap_or("N/A");
            let help_paragraph = Paragraph::new(Line::from(vec![
                Span::raw("Thread View (Serial: "),
                Span::styled(
                    thread_serial_display,
                    Style::default().fg(config.color_thread_serial),
                ),
                Span::raw(") | "),
                "Esc".bold().fg(config.color_keybind_key),
                ": exit thread view".into(),
            ]))
            .block(Block::default().borders(Borders::ALL).title("Thread View"));
            frame.render_widget(help_paragraph, chunks[1]);
        }
        Mode::GroupingSelection => {
            let area = centered_rect(50, 20, frame.area());
            let block = Block::default()
                .title("Select Grouping")
                .borders(Borders::ALL);
            frame.render_widget(Clear, area);
            frame.render_widget(&block, area);

            let inner_area = block.inner(area);

            let grouping_options = ["Sender", "Member", "Path", "Serial", "None"];
            let list_items: Vec<ListItem> = grouping_options
                .iter()
                .enumerate()
                .map(|(i, &option)| {
                    let mut spans = vec![];
                    // Add a small indicator for the current grouping type
                    if app.grouping_type.to_string() == option {
                        spans.push(Span::styled(
                            "● ",
                            Style::default().fg(config.color_grouping_active_indicator),
                        ));
                    } else {
                        spans.push(Span::raw("  "));
                    }
                    spans.push(Span::raw(format!(
                        "{}: {}",
                        match i {
                            0 => "s",
                            1 => "m",
                            2 => "p",
                            3 => "i",
                            4 => "n",
                            _ => "",
                        },
                        option
                    )));
                    ListItem::new(Line::from(spans))
                })
                .collect();

            let list = List::new(list_items)
                .block(Block::default())
                .highlight_symbol("> ")
                .highlight_style(
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .fg(config.color_selection_highlight_fg)
                        .bg(config.color_selection_highlight_bg),
                );

            frame.render_stateful_widget(list, inner_area, &mut app.grouping_selection_state);
        }
    }
}

// Helper function to format a `zbus::zvariant::Value` in a YAML-like, readable way.
pub fn format_value(value: &Value, config: &Config) -> Text<'static> {
    // Inner recursive function to handle nesting and indentation.
    fn format_recursive(
        value: &Value,
        indent: usize,
        prefix: &str,
        parent_alternating_index: usize,
        config: &Config,
    ) -> Vec<Line<'static>> {
        // Handle variants by unwrapping them and formatting the inner value directly.
        if let Value::Value(inner) = value {
            return format_recursive(inner, indent, prefix, parent_alternating_index, config);
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

        let mut lines = Vec::new();
        let base_style = if parent_alternating_index % 2 == 0 {
            Style::default()
        } else {
            Style::default().bg(config.color_default_stripe)
        };

        // Determine the style for the current line
        let current_item_style = match value {
            Value::Dict(_) => base_style.bg(config.color_dict),
            Value::Structure(_) => base_style.bg(config.color_struct),
            _ => base_style,
        };

        // Handle simple, single-line values first.
        match value {
            Value::U8(v) => lines.push(Line::styled(
                format!("{}{} [u8]: {}", indent_str, prefix, v),
                current_item_style,
            )),
            Value::I16(v) => lines.push(Line::styled(
                format!("{}{} [i16]: {}", indent_str, prefix, v),
                current_item_style,
            )),
            Value::U16(v) => lines.push(Line::styled(
                format!("{}{} [u16]: {}", indent_str, prefix, v),
                current_item_style,
            )),
            Value::I32(v) => lines.push(Line::styled(
                format!("{}{} [i32]: {}", indent_str, prefix, v),
                current_item_style,
            )),
            Value::U32(v) => lines.push(Line::styled(
                format!("{}{} [u32]: {}", indent_str, prefix, v),
                current_item_style,
            )),
            Value::I64(v) => lines.push(Line::styled(
                format!("{}{} [i64]: {}", indent_str, prefix, v),
                current_item_style,
            )),
            Value::U64(v) => lines.push(Line::styled(
                format!("{}{} [u64]: {}", indent_str, prefix, v),
                current_item_style,
            )),
            Value::F64(v) => lines.push(Line::styled(
                format!("{}{} [f64]: {}", indent_str, prefix, v),
                current_item_style,
            )),
            Value::Bool(v) => lines.push(Line::styled(
                format!("{}{} [bool]: {}", indent_str, prefix, v),
                current_item_style,
            )),
            Value::Str(s) => lines.push(Line::styled(
                format!("{}{} [str]: \"{}\"", indent_str, prefix, s),
                current_item_style,
            )),
            Value::Signature(s) => lines.push(Line::styled(
                format!("{}{} [signature]: '{}'", indent_str, prefix, s),
                current_item_style,
            )),
            Value::ObjectPath(p) => lines.push(Line::styled(
                format!("{}{} [object-path]: {}", indent_str, prefix, p.as_str()),
                current_item_style,
            )),
            Value::Fd(f) => lines.push(Line::styled(
                format!("{}{} [fd]: {:?}", indent_str, prefix, f),
                current_item_style,
            )),
            // This case is now reachable for `Value::Value`
            _ => {
                // Continue to complex types
                lines.push(Line::styled(
                    format!("{}{} [{}]:", indent_str, prefix, get_value_type_str(value)),
                    current_item_style,
                ));

                match value {
                    Value::Array(arr) => {
                        if arr.is_empty() {
                            lines.pop(); // Remove the "[array]:" line
                            lines.push(Line::styled(
                                format!("{}{} [array]: []", indent_str, prefix),
                                current_item_style,
                            ));
                        } else {
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
                                lines.pop(); // Remove the "[array]:" line for this special case
                                lines.push(Line::styled(
                                    format!("{}{} [struct[]]:", indent_str, prefix),
                                    current_item_style,
                                ));
                                let key_indent_str = "  ".repeat(indent + 1);

                                for (i, item) in arr.iter().enumerate() {
                                    if let Value::Structure(s) = item {
                                        if let (Value::Str(key), val) =
                                            (&s.fields()[0], &s.fields()[1])
                                        {
                                            lines.push(Line::styled(
                                                format!("{}{}:", key_indent_str, key),
                                                current_item_style,
                                            ));
                                            lines.extend(format_recursive(
                                                val,
                                                indent + 2,
                                                "",
                                                i,
                                                config,
                                            ));
                                        }
                                    }
                                }
                            } else {
                                // Special Case 2: Homogeneous array of simple types.
                                let first_val = &arr[0];
                                let is_simple_type = !matches!(
                                    first_val,
                                    Value::Array(_)
                                        | Value::Structure(_)
                                        | Value::Dict(_)
                                        | Value::Value(_)
                                );

                                if is_simple_type {
                                    let first_type_str = get_value_type_str(first_val);
                                    let all_same_simple_type = arr
                                        .iter()
                                        .skip(1)
                                        .all(|v| get_value_type_str(v) == first_type_str);

                                    if all_same_simple_type {
                                        if first_type_str == "u8" {
                                            lines.pop(); // Remove the "[array]:" line for this special case
                                            let bytes: Vec<u8> = arr
                                                .iter()
                                                .filter_map(|v| {
                                                    if let Value::U8(b) = v {
                                                        Some(*b)
                                                    } else {
                                                        None
                                                    }
                                                })
                                                .collect();

                                            lines.push(Line::styled(
                                                format!("{}{} [ay (u8[])]:", indent_str, prefix),
                                                current_item_style,
                                            ));
                                            let item_indent_str = "  ".repeat(indent + 1);

                                            for (i, chunk) in bytes.chunks(16).enumerate() {
                                                let mut line_text = String::new();
                                                // 1. Offset
                                                line_text.push_str(&format!(
                                                    "{}{:08x}: ",
                                                    item_indent_str,
                                                    i * 16
                                                ));

                                                // 2. Hex values
                                                let mut hex_part = String::new();
                                                for &byte in chunk {
                                                    hex_part.push_str(&format!("{:02x} ", byte));
                                                }
                                                line_text.push_str(&hex_part);

                                                // Add padding if the chunk is smaller than 16
                                                if chunk.len() < 16 {
                                                    for _ in 0..(16 - chunk.len()) {
                                                        line_text.push_str("   ");
                                                    }
                                                }

                                                // 3. ASCII representation
                                                let ascii_part: String = chunk
                                                    .iter()
                                                    .map(|&b| {
                                                        if b >= 0x20 && b <= 0x7e {
                                                            b as char
                                                        } else {
                                                            '.'
                                                        }
                                                    })
                                                    .collect();
                                                line_text.push_str(&format!(" |{}|", ascii_part));
                                                lines.push(Line::styled(
                                                    line_text,
                                                    current_item_style,
                                                ));
                                            }
                                        } else {
                                            // Compact display for other simple types
                                            lines.pop(); // Remove the "[array]:" line
                                            let items_str: Vec<String> = arr
                                                .iter()
                                                .map(|item| {
                                                    match item {
                                                        Value::Str(s) => format!("\"{}\"", s),
                                                        Value::Signature(s) => format!("'{}'", s),
                                                        _ => item.to_string(), // Uses Display impl for primitives
                                                    }
                                                })
                                                .collect();
                                            lines.push(Line::styled(
                                                format!(
                                                    "{}{} [{}[]]: [{}]",
                                                    indent_str,
                                                    prefix,
                                                    first_type_str,
                                                    items_str.join(", ")
                                                ),
                                                current_item_style,
                                            ));
                                        }
                                    } else {
                                        // Fallback for heterogeneous simple types array
                                        for (i, item) in arr.iter().enumerate() {
                                            lines.extend(format_recursive(
                                                item,
                                                indent + 1,
                                                &i.to_string(),
                                                i,
                                                config,
                                            ));
                                        }
                                    }
                                } else {
                                    // Fallback for all other array types (heterogeneous or complex elements).
                                    for (i, item) in arr.iter().enumerate() {
                                        lines.extend(format_recursive(
                                            item,
                                            indent + 1,
                                            &i.to_string(),
                                            i,
                                            config,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    Value::Structure(s) => {
                        if s.fields().is_empty() {
                            lines.pop(); // Remove the "[struct]:" line
                            lines.push(Line::styled(
                                format!("{}{} [struct]: (empty)", indent_str, prefix),
                                current_item_style,
                            ));
                        } else {
                            for (i, field) in s.fields().iter().enumerate() {
                                lines.extend(format_recursive(
                                    field,
                                    indent + 1,
                                    &format!("i_{}", i + 1),
                                    i, // Pass current index for alternating
                                    config,
                                ));
                            }
                        }
                    }
                    Value::Dict(d) => {
                        if d.iter().count() == 0 {
                            lines.pop(); // Remove the "[dict]:" line
                            lines.push(Line::styled(
                                format!("{}{} [dict]: {{}}", indent_str, prefix),
                                current_item_style,
                            ));
                        } else {
                            let mut entries: Vec<(String, &Value)> = Vec::new();
                            let mut max_key_len = 0;

                            for (k, v) in d.iter() {
                                let key_str = match k {
                                    Value::Str(s) => s.to_string(),
                                    _ => format!("{:?}", k).trim_matches('"').to_string(),
                                };
                                max_key_len = max_key_len.max(key_str.len());
                                entries.push((key_str, v));
                            }

                            let inner_indent_for_value = indent + 1;
                            let inner_indent_str = "  ".repeat(inner_indent_for_value);

                            for (i, (key_str, val)) in entries.into_iter().enumerate() {
                                let padding_for_key =
                                    " ".repeat(max_key_len.saturating_sub(key_str.len()));

                                let formatted_value_lines = format_recursive(val, 0, "", i, config);

                                if formatted_value_lines.is_empty() {
                                    lines.push(Line::styled(
                                        format!(
                                            "{}{} {}:",
                                            inner_indent_str, key_str, padding_for_key
                                        ),
                                        current_item_style,
                                    ));
                                } else {
                                    // First line of value
                                    let mut first_line_text = formatted_value_lines[0]
                                        .spans
                                        .iter()
                                        .map(|s| s.content.to_string())
                                        .collect::<String>();
                                    // Remove any leading spaces from the first line of the value, as the key already provides indentation.
                                    first_line_text = first_line_text.trim_start().to_string();

                                    lines.push(Line::styled(
                                        format!(
                                            "{}{} {}: {}",
                                            inner_indent_str,
                                            key_str,
                                            padding_for_key,
                                            first_line_text
                                        ),
                                        current_item_style,
                                    ));

                                    let value_start_col =
                                        (inner_indent_for_value * 2) + max_key_len + 2;
                                    for line in formatted_value_lines.into_iter().skip(1) {
                                        let line_content = line
                                            .spans
                                            .iter()
                                            .map(|s| s.content.to_string())
                                            .collect::<String>();
                                        lines.push(Line::styled(
                                            format!(
                                                "{}{}",
                                                " ".repeat(value_start_col),
                                                line_content
                                            ),
                                            current_item_style,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    _ => {
                        lines.push(Line::styled(
                            format!("{}{:?}", "  ".repeat(indent + 1), value),
                            current_item_style,
                        ));
                    }
                }
            }
        }
        lines
    }

    let mut all_lines: Vec<Line<'static>> = Vec::new();
    // Special handling for top-level `Structure` to match desired output format.
    if let Value::Structure(s) = value {
        for (i, field) in s.fields().iter().enumerate() {
            all_lines.extend(format_recursive(
                field,
                0,
                &format!("i_{}", i + 1),
                i,
                config,
            ));
        }
    } else {
        // Fallback for any non-Structure top-level value.
        all_lines.extend(format_recursive(value, 0, "value", 0, config));
    }
    Text::from(all_lines)
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
