use ratatui::prelude::*;

// color config
pub struct Config {
    pub color_dict: Color,
    pub color_struct: Color,
    pub color_default_stripe: Color,
    pub color_timestamp_normal: Color,
    pub color_timestamp_details: Color,
    pub color_stream_session: Color,
    pub color_stream_system: Color,
    pub color_sender_normal: Color,
    pub color_sender_details: Color,
    pub color_member_normal: Color,
    pub color_member_details: Color,
    pub color_path_normal: Color,
    pub color_path_details: Color,
    pub color_status_message: Color,
    pub color_keybind_text: Color,
    pub color_keybind_key: Color,
    pub color_thread_serial: Color,
    pub color_grouping_active_indicator: Color,
    pub color_selection_highlight_bg: Color,
    pub color_selection_highlight_fg: Color,
    pub color_autofilter_value: Color,
    pub color_ticker: Color,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            color_dict: Color::Rgb(20, 20, 40),   // Dark Blue
            color_struct: Color::Rgb(40, 20, 40), // Dark Magenta
            color_default_stripe: Color::DarkGray,
            color_timestamp_normal: Color::Yellow,
            color_timestamp_details: Color::White,
            color_stream_session: Color::Cyan,
            color_stream_system: Color::LightMagenta,
            color_sender_normal: Color::Green,
            color_sender_details: Color::White,
            color_member_normal: Color::Blue,
            color_member_details: Color::White,
            color_path_normal: Color::Magenta,
            color_path_details: Color::White,
            color_status_message: Color::Yellow,
            color_keybind_text: Color::Reset,
            color_keybind_key: Color::Cyan,
            color_thread_serial: Color::LightBlue,
            color_grouping_active_indicator: Color::LightGreen,
            color_selection_highlight_bg: Color::Reset,
            color_selection_highlight_fg: Color::Cyan,
            color_autofilter_value: Color::Green,
            color_ticker: Color::Rgb(255, 255, 0),
        }
    }
}
