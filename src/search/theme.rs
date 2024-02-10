use ratatui::prelude::Color;

/// Theme configurations
pub struct ThemeConfig {
    /// Color of the titles (i.e. block titles)
    pub title: Color,
    /// Color of the borders
    pub border: Color,
    /// Color of the labels (i.e. field names)
    pub label: Color,
    /// Color of the values (i.e. field values)
    pub value: Color,
}
