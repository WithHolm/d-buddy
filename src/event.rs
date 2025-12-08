use crate::bus::BusType;
use crate::config::Config;
use crate::state::{App, Mode};
use anyhow::Result;
use arboard::Clipboard;
use crossterm::event::{Event, KeyCode};
use ratatui::prelude::*;
use ratatui::text::{Line, Span, Text};
use std::sync::{Arc, Mutex};

use tokio::fs;
use tui_input::{backend::crossterm as input_backend, Input};
use zbus::zvariant::{Structure, Value};

//check for user input/key presses
pub async fn handle_event(
    app: &mut App,
    config: &Config,
    event: Event,
    clipboard_arc: Arc<Mutex<Clipboard>>,
) -> Result<bool> {
    if let Event::Key(key) = event {
        match app.mode {
            Mode::Normal => {
                if !app.status_message.is_empty() {
                    app.status_message.clear();
                }

                match key.code {
                    KeyCode::Char('q') => {
                        if !app.show_details {
                            return Ok(true);
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
                    KeyCode::Char('T') => {
                        app.use_relative_time = !app.use_relative_time;
                    }
                    KeyCode::Char('t') => {
                        if let Some(selected) = app.list_state.selected() {
                            if let Some(item) = app.filtered_and_sorted_items.get(selected) {
                                app.conversation_serial = Some(item.serial.clone());
                                app.mode = Mode::ConversationView;
                            }
                        }
                    }
                    KeyCode::Char('x') => {
                        app.clear_session_items();
                        app.clear_system_items();
                        app.filtered_and_sorted_items.clear();
                        app.list_state.select(None);
                        app.status_message = "Message lists cleared.".to_string();
                    }
                    KeyCode::Char('g') => {
                        app.mode = Mode::GroupingSelection;
                        if app.grouping_selection_state.selected().is_none() {
                            app.grouping_selection_state.select(Some(0));
                        }
                    }
                    KeyCode::Char('f') => {
                        app.mode = Mode::Filtering;
                    }
                    KeyCode::Char('F') => {
                        app.follow = !app.follow;
                        app.status_message = if app.follow {
                            "Follow mode enabled.".to_string()
                        } else {
                            "Follow mode disabled.".to_string()
                        };
                    }
                    KeyCode::Up => {
                        if !app.filtered_and_sorted_items.is_empty() {
                            app.follow = false;
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
                        if !app.filtered_and_sorted_items.is_empty() {
                            app.follow = false;
                            let current_selected_before = app.list_state.selected();
                            tracing::debug!(
                                "Down: current_selected_before = {:?}",
                                current_selected_before
                            );

                            let i = match current_selected_before {
                                Some(val) => (val + 1).min(app.filtered_and_sorted_items.len() - 1),
                                None => 0,
                            };
                            tracing::debug!("Down: calculated_i = {}", i);

                            app.list_state.select(Some(i));
                            tracing::debug!(
                                "Down: current_selected_after_select = {:?}",
                                app.list_state.selected()
                            );

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
                            if let Some(item) = app.filtered_and_sorted_items.get(selected) {
                                let bus_type = match app.stream {
                                    BusType::Session => "--session",
                                    BusType::System => "--system",
                                    BusType::Both => "--session", // Default
                                };
                                let command = format!(
                                    "dbus-send {} --dest={} {} <interface>.<member>",
                                    bus_type, item.sender, item.path
                                );

                                let clipboard_arc_clone = clipboard_arc.clone();
                                let command_clone = command.clone();
                                let result = tokio::task::spawn_blocking(move || {
                                    clipboard_arc_clone.lock().unwrap().set_text(command_clone)
                                })
                                .await;

                                match result {
                                    Ok(Ok(_)) => {
                                        app.status_message =
                                            format!("Copied to clipboard: {}", command);
                                    }
                                    Ok(Err(e)) => {
                                        app.status_message =
                                            format!("Failed to copy to clipboard: {}", e);
                                    }
                                    Err(e) => {
                                        app.status_message = format!("Copy task failed: {}", e);
                                    }
                                }
                            }
                        }
                    }
                    KeyCode::Char('c') => {
                        if app.show_details {
                            let text_to_copy = app.detail_text.to_string();
                            let file_path = "/tmp/d-buddy-details.txt";
                            let file_write_status =
                                match fs::write(file_path, text_to_copy.as_bytes()).await {
                                    Ok(_) => format!("Saved to {}", file_path),
                                    Err(e) => format!("Failed to save to file: {}", e),
                                };

                            let clipboard_arc_clone = clipboard_arc.clone();
                            let result = tokio::task::spawn_blocking(move || {
                                clipboard_arc_clone.lock().unwrap().set_text(text_to_copy)
                            })
                            .await;

                            let clipboard_status = match result {
                                Ok(Ok(_)) => "Copied to clipboard!".to_string(),
                                Ok(Err(e)) => format!("Copy failed: {}", e),
                                Err(e) => format!("Copy task failed: {}", e),
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
                            app.follow = false;
                            app.detail_scroll_request = Some(10);
                        }
                    }
                    KeyCode::PageUp => {
                        if app.show_details {
                            app.follow = false;
                            app.detail_scroll_request = Some(-10);
                        }
                    }
                    _ => {} // Ignore other keys
                }
            }
            Mode::Filtering => {
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
                    KeyCode::Tab => {
                        // If there's a selected item in the main list,
                        // initialize autofilter_selection_state and enter AutoFilterSelection mode.
                        if app.list_state.selected().is_some() {
                            app.mode = Mode::AutoFilterSelection;
                            // Ensure a selection is made when entering the autofilter selection mode
                            // Default to the first option (sender)
                            app.autofilter_selection_state.select(Some(0));
                        }
                    }
                    KeyCode::Esc => {
                        app.input.reset();
                        app.filter_criteria.clear(); // Clear all field filters
                        app.mode = Mode::Normal;
                    }
                    _ => {
                        if let Some(req) = input_backend::to_input_request(&event) {
                            app.input.handle(req);
                        }
                    }
                }
            }
            Mode::GroupingSelection => {
                let all_grouping_options = [
                    crate::bus::GroupingType::Sender,
                    crate::bus::GroupingType::Member,
                    crate::bus::GroupingType::Path,
                    crate::bus::GroupingType::Serial,
                    crate::bus::GroupingType::None,
                ];
                let max_index = all_grouping_options.len() - 1;

                match key.code {
                    KeyCode::Up => {
                        let i = match app.grouping_selection_state.selected() {
                            Some(i) => i.saturating_sub(1),
                            None => 0,
                        };
                        app.grouping_selection_state.select(Some(i));
                    }
                    KeyCode::Down => {
                        let i = match app.grouping_selection_state.selected() {
                            Some(i) => (i + 1).min(max_index),
                            None => 0,
                        };
                        app.grouping_selection_state.select(Some(i));
                    }
                    KeyCode::Char(' ') => {
                        if let Some(selected_index) = app.grouping_selection_state.selected() {
                            let selected_grouping_type = all_grouping_options[selected_index];

                            if selected_grouping_type == crate::bus::GroupingType::None {
                                // If "None" is selected, clear all other groupings and set only "None"
                                app.grouping_keys.clear();
                                app.grouping_keys.push(crate::bus::GroupingType::None);
                            } else {
                                // Toggle other grouping types
                                if let Some(pos) = app
                                    .grouping_keys
                                    .iter()
                                    .position(|&gt| gt == selected_grouping_type)
                                {
                                    // Already selected, remove it
                                    app.grouping_keys.remove(pos);
                                }
                                // Not selected, add it
                                else {
                                    app.grouping_keys.push(selected_grouping_type);
                                    // Remove GroupingType::None if another type is added
                                    app.grouping_keys
                                        .retain(|&gt| gt != crate::bus::GroupingType::None);
                                }

                                // If no grouping keys are left, default to None
                                if app.grouping_keys.is_empty() {
                                    app.grouping_keys.push(crate::bus::GroupingType::None);
                                }
                            }

                            // Sort grouping keys for consistent order (e.g., Sender, Member, Path, Serial)
                            app.grouping_keys.sort_by_key(|&gt| match gt {
                                crate::bus::GroupingType::Sender => 0,
                                crate::bus::GroupingType::Member => 1,
                                crate::bus::GroupingType::Path => 2,
                                crate::bus::GroupingType::Serial => 3,
                                crate::bus::GroupingType::None => 4,
                            });
                            // Make sure None is always at the end if it's present with other keys
                            if app.grouping_keys.len() > 1
                                && app.grouping_keys.contains(&crate::bus::GroupingType::None)
                            {
                                app.grouping_keys
                                    .retain(|&gt| gt != crate::bus::GroupingType::None);
                                app.grouping_keys.push(crate::bus::GroupingType::None);
                            }
                        }
                        // app.mode = Mode::Normal;
                    }
                    KeyCode::Esc | KeyCode::Char('g') => {
                        app.mode = Mode::Normal;
                    }
                    _ => {} // Ignore other keys
                }
            }
            Mode::AutoFilterSelection => {
                let autofilter_options = ["sender", "member", "path", "serial", "reply_serial"];
                let max_index = autofilter_options.len() - 1;

                match key.code {
                    KeyCode::Up => {
                        let i = match app.autofilter_selection_state.selected() {
                            Some(i) => i.saturating_sub(1),
                            None => 0,
                        };
                        app.autofilter_selection_state.select(Some(i));
                    }
                    KeyCode::Down => {
                        let i = match app.autofilter_selection_state.selected() {
                            Some(i) => (i + 1).min(max_index),
                            None => 0,
                        };
                        app.autofilter_selection_state.select(Some(i));
                    }
                    KeyCode::Enter => {
                        if let Some(selected_option_index) =
                            app.autofilter_selection_state.selected()
                        {
                            if let Some(selected_message_index) = app.list_state.selected() {
                                if let Some(item) =
                                    app.filtered_and_sorted_items.get(selected_message_index)
                                {
                                    let field_name = autofilter_options[selected_option_index];
                                    let field_value: std::borrow::Cow<'_, str> = match field_name {
                                        "sender" => item.sender_display(),
                                        "member" => item.member.as_str().into(),
                                        "path" => item.path.as_str().into(),
                                        "serial" => item.serial.as_str().into(),
                                        "reply_serial" => item.reply_serial.as_str().into(),
                                        _ => "".into(),
                                    };
                                    app.input =
                                        Input::from(format!("{}={}", field_name, field_value));
                                }
                            }
                        }
                        app.autofilter_selection_state.select(None); // Clear selection
                        app.mode = Mode::Filtering; // Go back to filtering input
                    }
                    KeyCode::Esc => {
                        app.autofilter_selection_state.select(None); // Clear selection
                        app.mode = Mode::Filtering; // Go back to filtering input
                    }
                    _ => {} // Ignore other keys
                }
            }
            Mode::ConversationView => {
                if key.code == KeyCode::Esc {
                    app.conversation_serial = None;
                    app.mode = Mode::Normal;
                }
            }
        }
    }
    Ok(false)
}

/// A helper function to generate the detail text for the currently selected message.
fn update_detail_text(app: &mut App, config: &Config) {
    if let Some(selected) = app.list_state.selected() {
        if let Some(item) = app.filtered_and_sorted_items.get(selected) {
            let mut header_lines: Vec<Line> = Vec::new();

            let recipient_info = if item.receiver.is_empty() {
                String::new()
            } else {
                format!(" -> {}", item.receiver_display().into_owned())
            };
            let reply_serial_info = if item.is_reply && !item.reply_serial.is_empty() {
                format!("->{}", item.reply_serial)
            } else {
                String::new()
            };

            header_lines.push(Line::from(vec![
                Span::styled(
                    item.sender_display().into_owned(),
                    Style::default().fg(config.color_sender_normal),
                ),
                Span::raw(recipient_info),
                Span::raw("|"),
                Span::styled(
                    item.serial.clone(),
                    match item.stream_type {
                        BusType::Session => Style::default().fg(config.color_stream_session),
                        BusType::System => Style::default().fg(config.color_stream_system),
                        BusType::Both => Style::default().fg(config.color_timestamp_normal), // Fallback
                    },
                ),
                Span::raw(reply_serial_info),
                Span::raw("|"),
                Span::styled(
                    item.member.clone(),
                    Style::default().fg(config.color_member_normal),
                ),
                Span::raw(":"),
                Span::styled(
                    item.path.clone(),
                    Style::default().fg(config.color_path_normal),
                ),
            ]));
            header_lines.push(Line::from(vec![Span::raw("")])); // Empty line for spacing

            if !item.app_path.is_empty() {
                header_lines.push(Line::from(vec![
                    Span::raw("Sender Path: "),
                    Span::styled(
                        item.app_path.clone(),
                        Style::default().fg(config.color_path_normal),
                    ),
                ]));
            }

            if !item.app_args.is_empty() {
                header_lines.push(Line::from(vec![
                    Span::raw("Sender Args: "),
                    Span::styled(
                        item.app_args.join(" "),
                        Style::default().fg(config.color_member_normal),
                    ),
                ]));
            }
            if !item.receiver_app_path.is_empty() {
                header_lines.push(Line::from(vec![
                    Span::raw("Receiver Path: "),
                    Span::styled(
                        item.receiver_app_path.clone(),
                        Style::default().fg(config.color_path_normal),
                    ),
                ]));
            }

            if !item.receiver_app_args.is_empty() {
                header_lines.push(Line::from(vec![
                    Span::raw("Receiver Args: "),
                    Span::styled(
                        item.receiver_app_args.join(" "),
                        Style::default().fg(config.color_member_normal),
                    ),
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
                        Ok(structure) => crate::ui::format_value(&Value::from(structure), config),
                        Err(_) => match body.deserialize::<Value>() {
                            Ok(value) => crate::ui::format_value(&value, config),
                            Err(e) => Text::from(format!(
                                "Failed to deserialize body.\n\nSignature: {}
Error: {:#?}",
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
            header_text.extend(detail_text);
            app.detail_text = header_text;

            app.detail_scroll = 0;
        }
    }
}
