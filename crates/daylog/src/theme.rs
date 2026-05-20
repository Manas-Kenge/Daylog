use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{BorderType, Padding};

pub const PANEL_BORDER: BorderType = BorderType::Rounded;
pub const PANEL_PADDING: Padding = Padding::horizontal(1);
pub const PANEL_PADDING_TIGHT: Padding = Padding::new(1, 1, 0, 0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Truecolor,
    Color256,
    Ansi16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    Wide,
    Narrow,
    Stacked,
}

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub tier: Tier,
    pub bg: Color,
    pub fg: Color,
    pub dim: Color,
    pub border_dim: Color,
    /// Reserved for focused-panel borders (not wired yet).
    pub border_active: Color,
    pub ember: Color,
    pub error: Color,
    pub chart_1: Color,
    pub chart_2: Color,
    pub chart_3: Color,
    pub chart_4: Color,
    pub chart_5: Color,
    chart_2_extra: Modifier,
}

impl Theme {
    pub fn detect() -> Self {
        let colorterm = std::env::var("COLORTERM").ok();
        let term = std::env::var("TERM").ok();
        Self::from_env_pair(colorterm.as_deref(), term.as_deref())
    }

    pub fn from_env_pair(colorterm: Option<&str>, term: Option<&str>) -> Self {
        let tier = match colorterm {
            Some("truecolor") | Some("24bit") => Tier::Truecolor,
            _ => match term {
                Some(t) if t.contains("256color") => Tier::Color256,
                _ => Tier::Ansi16,
            },
        };
        match tier {
            Tier::Truecolor => Self::truecolor(),
            Tier::Color256 => Self::color256(),
            Tier::Ansi16 => Self::ansi16(),
        }
    }

    fn truecolor() -> Self {
        Self {
            tier: Tier::Truecolor,
            bg: Color::Rgb(0, 0, 0),
            fg: Color::Rgb(251, 251, 251),
            dim: Color::Rgb(176, 176, 176),
            border_dim: Color::Rgb(64, 64, 64),
            border_active: Color::Rgb(229, 154, 110),
            ember: Color::Rgb(229, 154, 110),
            error: Color::Rgb(228, 113, 99),
            chart_1: Color::Rgb(238, 159, 99),
            chart_2: Color::Rgb(218, 191, 108),
            chart_3: Color::Rgb(141, 189, 142),
            chart_4: Color::Rgb(115, 180, 202),
            chart_5: Color::Rgb(126, 131, 201),
            chart_2_extra: Modifier::empty(),
        }
    }

    fn color256() -> Self {
        Self {
            tier: Tier::Color256,
            bg: Color::Indexed(0),
            fg: Color::Indexed(231),
            dim: Color::Indexed(244),
            border_dim: Color::Indexed(239),
            border_active: Color::Indexed(173),
            ember: Color::Indexed(173),
            error: Color::Indexed(167),
            chart_1: Color::Indexed(215),
            chart_2: Color::Indexed(186),
            chart_3: Color::Indexed(108),
            chart_4: Color::Indexed(110),
            chart_5: Color::Indexed(104),
            chart_2_extra: Modifier::empty(),
        }
    }

    fn ansi16() -> Self {
        Self {
            tier: Tier::Ansi16,
            bg: Color::Black,
            fg: Color::White,
            dim: Color::White,
            border_dim: Color::Black,
            border_active: Color::Yellow,
            ember: Color::Yellow,
            error: Color::Red,
            chart_1: Color::Yellow,
            chart_2: Color::Yellow,
            chart_3: Color::Green,
            chart_4: Color::Cyan,
            chart_5: Color::Magenta,
            chart_2_extra: Modifier::BOLD,
        }
    }

    pub fn layout_mode(width: u16) -> LayoutMode {
        if width >= 100 {
            LayoutMode::Wide
        } else if width >= 80 {
            LayoutMode::Narrow
        } else {
            LayoutMode::Stacked
        }
    }

    pub fn spectrum_color(&self, hour: u8) -> Color {
        match hour {
            0..=4 => self.chart_1,
            5..=9 => self.chart_2,
            10..=14 => self.chart_3,
            15..=19 => self.chart_4,
            20..=23 => self.chart_5,
            _ => self.chart_3,
        }
    }

    pub fn dim_style(&self) -> Style {
        Style::default().fg(self.dim).add_modifier(Modifier::DIM)
    }

    pub fn ember_style(&self) -> Style {
        Style::default().fg(self.ember)
    }

    pub fn error_style(&self) -> Style {
        Style::default().fg(self.error)
    }

    pub fn border_dim_style(&self) -> Style {
        Style::default()
            .fg(self.border_dim)
            .add_modifier(Modifier::DIM)
    }

    pub fn active_tab_style(&self) -> Style {
        Style::default()
            .fg(self.bg)
            .bg(self.ember)
            .add_modifier(Modifier::BOLD)
    }

    pub fn inactive_tab_style(&self) -> Style {
        Style::default().add_modifier(Modifier::DIM)
    }

    pub fn active_chip_style(&self) -> Style {
        Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
    }

    pub fn inactive_chip_style(&self) -> Style {
        Style::default().add_modifier(Modifier::DIM)
    }

    pub fn kpi_label_style(&self) -> Style {
        // DIM-on-dim disappears on linux console.
        Style::default().fg(self.dim)
    }

    pub fn kpi_value_style(&self) -> Style {
        Style::default().fg(self.fg).add_modifier(Modifier::BOLD)
    }

    pub fn chart_2_style(&self) -> Style {
        Style::default().fg(self.chart_2).add_modifier(self.chart_2_extra)
    }

    pub fn category_color(&self, root: &str) -> Color {
        match root {
            "Work" | "Programming" => self.chart_1,
            "Comms" => self.chart_2,
            "Media" => self.chart_3,
            "Browsing" => self.chart_4,
            "Documents" => self.chart_5,
            _ => self.dim,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_truecolor_from_colorterm() {
        let t = Theme::from_env_pair(Some("truecolor"), Some("xterm-256color"));
        assert_eq!(t.tier, Tier::Truecolor);
    }

    #[test]
    fn detect_truecolor_from_24bit_alias() {
        let t = Theme::from_env_pair(Some("24bit"), Some("xterm-256color"));
        assert_eq!(t.tier, Tier::Truecolor);
    }

    #[test]
    fn detect_color256_when_no_colorterm() {
        let t = Theme::from_env_pair(None, Some("xterm-256color"));
        assert_eq!(t.tier, Tier::Color256);
    }

    #[test]
    fn detect_ansi16_floor() {
        assert_eq!(Theme::from_env_pair(None, None).tier, Tier::Ansi16);
        assert_eq!(
            Theme::from_env_pair(None, Some("dumb")).tier,
            Tier::Ansi16
        );
    }

    #[test]
    fn detect_truecolor_takes_precedence_over_term() {
        let t = Theme::from_env_pair(Some("truecolor"), Some("xterm"));
        assert_eq!(t.tier, Tier::Truecolor);
    }

    #[test]
    fn spectrum_band_assignments() {
        let t = Theme::from_env_pair(Some("truecolor"), None);
        assert_eq!(t.spectrum_color(0), t.chart_1);
        assert_eq!(t.spectrum_color(7), t.chart_2);
        assert_eq!(t.spectrum_color(12), t.chart_3);
        assert_eq!(t.spectrum_color(17), t.chart_4);
        assert_eq!(t.spectrum_color(22), t.chart_5);
        assert_eq!(t.spectrum_color(4), t.chart_1);
        assert_eq!(t.spectrum_color(5), t.chart_2);
        assert_eq!(t.spectrum_color(9), t.chart_2);
        assert_eq!(t.spectrum_color(10), t.chart_3);
        assert_eq!(t.spectrum_color(14), t.chart_3);
        assert_eq!(t.spectrum_color(15), t.chart_4);
        assert_eq!(t.spectrum_color(19), t.chart_4);
        assert_eq!(t.spectrum_color(20), t.chart_5);
    }

    #[test]
    fn layout_mode_breakpoints() {
        assert_eq!(Theme::layout_mode(120), LayoutMode::Wide);
        assert_eq!(Theme::layout_mode(100), LayoutMode::Wide);
        assert_eq!(Theme::layout_mode(99), LayoutMode::Narrow);
        assert_eq!(Theme::layout_mode(80), LayoutMode::Narrow);
        assert_eq!(Theme::layout_mode(79), LayoutMode::Stacked);
        assert_eq!(Theme::layout_mode(0), LayoutMode::Stacked);
    }

    #[test]
    fn style_helpers_compose_correctly() {
        let t = Theme::from_env_pair(Some("truecolor"), None);
        assert!(t.kpi_value_style().add_modifier.contains(Modifier::BOLD));
        assert!(!t.kpi_label_style().add_modifier.contains(Modifier::DIM));
        assert_eq!(t.kpi_label_style().fg, Some(t.dim));
        let active = t.active_tab_style();
        assert_eq!(active.bg, Some(t.ember));
        assert_eq!(active.fg, Some(t.bg));
        assert!(active.add_modifier.contains(Modifier::BOLD));
        assert!(!active.add_modifier.contains(Modifier::DIM));
    }
}
