use crate::bus::{BusType, Item};
use ratatui::{
    style::Stylize,
    text::{Line, Text},
    widgets::ListState,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tui_input::Input;

// Enum to define the current operating mode of the application
#[derive(PartialEq)]
pub enum Mode {
    Normal,              // Default mode for browsing D-Bus messages
    Filtering,           // Mode for entering a filter string
    AutoFilterSelection, // Mode for selecting autofilter field
    ThreadView,          // Mode for viewing a specific message thread
    GroupingSelection,   // Mode for selecting a grouping option
}

// Main application struct holding all the state
pub struct App {
    pub stream: BusType,
    pub messages: HashMap<BusType, Arc<Mutex<Vec<Item>>>>,
    pub filtered_and_sorted_items: Vec<Item>,
    pub list_state: ListState, // State of the message list widget (e.g., selected item)
    pub show_details: bool,    // Flag to indicate if message details popup should be shown
    pub mode: Mode,            // Current operating mode (Normal or Filtering)
    pub input: Input,          // Input buffer for the filtering text
    pub detail_text: Text<'static>, // The formatted string for the currently viewed detail
    pub detail_scroll: u16,    // The vertical scroll offset for the detail view
    pub status_message: String, // A temporary message to show in the status bar
    pub thread_serial: Option<String>,
    pub detail_scroll_request: Option<i32>,
    pub filter_criteria: HashMap<String, String>,
    pub grouping_keys: Vec<crate::bus::GroupingType>,
    pub grouping_selection_state: ListState,
    pub autofilter_selection_state: ListState,
    pub min_width: u16,
    pub min_height: u16,
    pub use_relative_time: bool,
    pub enable_lighting_strike: bool,

    // Cached static UI elements
    pub cached_filtering_key_hints: Option<Line<'static>>,
    pub cached_normal_details_key_hints: Option<Line<'static>>,
    pub cached_normal_key_hints: Option<Line<'static>>,
    pub cached_autofilter_selection_key_hints: Option<Line<'static>>,
    pub cached_thread_view_key_hints: Option<Line<'static>>,
    pub cached_grouping_selection_key_hints: Option<Line<'static>>,
    pub cached_console_too_small_message: Option<Line<'static>>,

    // Cached title elements
    pub cached_title_prefix: Option<Line<'static>>,
    pub cached_title_suffix: Option<Line<'static>>,
}

// Default implementation for the App struct
impl Default for App {
    fn default() -> Self {
        App {
            stream: BusType::Session,
            messages: HashMap::new(), // Initialize with an empty list of messages
            filtered_and_sorted_items: Vec::new(),
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
            grouping_keys: vec![crate::bus::GroupingType::None],
            grouping_selection_state: ListState::default(),
            autofilter_selection_state: ListState::default(),
            min_width: 20,
            min_height: 20,
            use_relative_time: false,
            enable_lighting_strike: false,

            // Initialize cached elements as None
            cached_filtering_key_hints: None,
            cached_normal_details_key_hints: None,
            cached_normal_key_hints: None,
            cached_autofilter_selection_key_hints: None,
            cached_thread_view_key_hints: None,
            cached_grouping_selection_key_hints: None,
            cached_console_too_small_message: None,
            cached_title_prefix: None,
            cached_title_suffix: None,
        }
    }
}

impl App {
    pub fn initialize_static_ui_elements(&mut self, config: &crate::config::Config) {
        // "Console too small" message
        self.cached_console_too_small_message = Some(Line::from(
            "Console is too small to display the application (q to quit)",
        ));

        // Filtering key hints
        self.cached_filtering_key_hints = Some(Line::from(vec![
            "Esc".bold().fg(config.color_keybind_key),
            ": clear | ".into(),
            "Enter".bold().fg(config.color_keybind_key),
            ": apply | ".into(),
            "Tab".bold().fg(config.color_keybind_key),
            ": autofilter".into(),
        ]));

        // Normal mode (details view open) key hints
        self.cached_normal_details_key_hints = Some(Line::from(vec![
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
        ]));

        // Normal mode (main view) key hints
        self.cached_normal_key_hints = Some(Line::from(vec![
            "q".bold().fg(config.color_keybind_key),
            ": quit | ".into(),
            "Tab".bold().fg(config.color_keybind_key),
            ": view | ".into(),
            "t".bold().fg(config.color_keybind_key),
            ": time | ".into(),
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
        ]));

        // AutoFilterSelection key hints
        self.cached_autofilter_selection_key_hints = Some(Line::from(vec![
            "Esc".bold().fg(config.color_keybind_key),
            ": cancel | ".into(),
            "Enter".bold().fg(config.color_keybind_key),
            ": select | ".into(),
            "↑".bold().fg(config.color_keybind_key),
            "/".dim(),
            "↓".bold().fg(config.color_keybind_key),
            ": navigate".into(),
        ]));

        // ThreadView key hints
        self.cached_thread_view_key_hints = Some(Line::from(vec![
            "Esc".bold().fg(config.color_keybind_key),
            ": exit thread view".into(),
        ]));

        // GroupingSelection key hints
        self.cached_grouping_selection_key_hints = Some(Line::from(vec![
            "Esc".bold().fg(config.color_keybind_key),
            ": cancel | ".into(),
            "Enter".bold().fg(config.color_keybind_key),
            ": select | ".into(),
            "↑".bold().fg(config.color_keybind_key),
            "/".dim(),
            "↓".bold().fg(config.color_keybind_key),
            ": navigate".into(),
        ]));

        // Title elements
        self.cached_title_prefix = Some(Line::from("D-Bus Signals ["));
        self.cached_title_suffix = Some(Line::from("]"));
    }
}
