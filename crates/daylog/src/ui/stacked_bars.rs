//! 7-column stacked-bar widget for the Week tab. Each column maps to one
//! day of the calendar week (Mon → Sun); each segment within a column maps
//! to a category root, coloured via `category_root_style`. Future days
//! (later this week) render the axis label only.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    widgets::Widget,
};

use crate::data::WeekDayBuckets;
use crate::theme::Theme;
use crate::ui::week::category_root_style;

const BLOCK: &str = "\u{2588}";
const DOT: &str = "\u{00b7}";
const WEEKDAY_LETTERS: [char; 7] = ['M', 'T', 'W', 'T', 'F', 'S', 'S'];

/// Render the stacked-bar chart into `area`. The caller owns the panel
/// block + title; this fn paints into the inner rect.
pub fn render(
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    days: Option<&[WeekDayBuckets]>,
    in_flight: bool,
) {
    if area.width < 12 || area.height < 4 {
        return;
    }

    let Some(days) = days else {
        if in_flight {
            paint_centered(area, buf, theme, "\u{2026}");
        }
        return;
    };
    if days.len() != 7 {
        return;
    }

    // Layout: left 4 cols = y-axis ticks, bottom 1 row = x-axis labels.
    let y_axis_w: u16 = 4;
    let x_axis_h: u16 = 1;
    if area.width <= y_axis_w + 7 || area.height <= x_axis_h + 1 {
        return;
    }
    let chart = Rect {
        x: area.x + y_axis_w,
        y: area.y,
        width: area.width - y_axis_w,
        height: area.height - x_axis_h,
    };
    let x_axis = Rect {
        x: chart.x,
        y: area.y + area.height - x_axis_h,
        width: chart.width,
        height: x_axis_h,
    };
    let y_axis = Rect {
        x: area.x,
        y: area.y,
        width: y_axis_w,
        height: chart.height,
    };

    // y-axis ticks. Ceil to next hour for stable visual scale.
    let max_secs = days
        .iter()
        .map(|d| d.total_active_secs)
        .fold(0.0_f64, f64::max);
    let column_max_secs = ceil_hour(max_secs).max(1.0);
    paint_y_axis(y_axis, buf, theme, column_max_secs);

    paint_x_axis(x_axis, buf, theme);

    let col_w = chart.width / 7;
    if col_w == 0 {
        return;
    }
    let bar_inner_w = col_w.saturating_sub(2).max(1);

    for (i, day) in days.iter().enumerate() {
        let col_x = chart.x + (i as u16) * col_w;
        let col = Rect {
            x: col_x,
            y: chart.y,
            width: col_w,
            height: chart.height,
        };
        paint_column(col, buf, theme, day, column_max_secs, bar_inner_w);
    }
}

fn paint_y_axis(area: Rect, buf: &mut Buffer, theme: &Theme, max_secs: f64) {
    let style = Style::default()
        .fg(theme.dim)
        .add_modifier(Modifier::DIM);
    let max_h = (max_secs / 3600.0).round() as u16;
    let half_h = (max_h / 2).max(1);
    if area.height >= 1 {
        let label = format!("{}h", max_h);
        write_str(buf, area.x, area.y, &label, style);
    }
    if area.height >= 3 {
        let mid_y = area.y + area.height / 2;
        let label = format!("{}h", half_h);
        write_str(buf, area.x, mid_y, &label, style);
    }
    if area.height >= 2 {
        let bot_y = area.y + area.height - 1;
        write_str(buf, area.x, bot_y, "0", style);
    }
}

fn paint_x_axis(area: Rect, buf: &mut Buffer, theme: &Theme) {
    let style = Style::default()
        .fg(theme.dim)
        .add_modifier(Modifier::DIM);
    let col_w = area.width / 7;
    if col_w == 0 {
        return;
    }
    for i in 0..7 {
        let x = area.x + (i as u16) * col_w + col_w / 2;
        let mut s = String::new();
        s.push(WEEKDAY_LETTERS[i]);
        write_str(buf, x, area.y, &s, style);
    }
}

fn paint_column(
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    day: &WeekDayBuckets,
    column_max_secs: f64,
    bar_inner_w: u16,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let pad_left = area.width.saturating_sub(bar_inner_w) / 2;
    let bar_x = area.x + pad_left;

    // Future days: nothing to paint. The x-axis label sits below `area`.
    if day.is_future {
        return;
    }

    let chart_h = area.height;
    if day.total_active_secs <= 0.0 {
        // Past day with no activity — render a single dim baseline dot so
        // the column reads as "tracked, empty" rather than "missing".
        let dim_style = Style::default()
            .fg(theme.dim)
            .add_modifier(Modifier::DIM);
        let baseline_y = area.y + chart_h - 1;
        for dx in 0..bar_inner_w {
            write_str(buf, bar_x + dx, baseline_y, DOT, dim_style);
        }
        return;
    }

    // Bar height in rows, scaled against the week's max.
    let bar_h_f = (day.total_active_secs / column_max_secs) * chart_h as f64;
    let bar_h = bar_h_f.round().max(1.0).min(chart_h as f64) as u16;

    // Allocate segment heights via largest-remainder rounding so they
    // sum exactly to bar_h. Segments stacked bottom-up; bottom = first
    // root (largest by ROOT_ORDER convention from data.rs).
    let segments = allocate_segments(&day.roots, day.total_active_secs, bar_h);

    let mut y_from_bottom: u16 = 0;
    let baseline_y = area.y + chart_h - 1;
    for (root, seg_h) in segments {
        if seg_h == 0 {
            continue;
        }
        let style = category_root_style(theme, &root);
        for r in 0..seg_h {
            let y = baseline_y - y_from_bottom - r;
            for dx in 0..bar_inner_w {
                write_str(buf, bar_x + dx, y, BLOCK, style);
            }
        }
        y_from_bottom += seg_h;
    }
}

/// Allocate `bar_h` integer rows across roots in proportion to each
/// root's share of `total`. Uses largest-remainder rounding so the sum is
/// exactly `bar_h`. Order of returned segments matches `roots` order.
pub(crate) fn allocate_segments(
    roots: &[(String, f64)],
    total: f64,
    bar_h: u16,
) -> Vec<(String, u16)> {
    if bar_h == 0 || total <= 0.0 || roots.is_empty() {
        return roots.iter().map(|(n, _)| (n.clone(), 0)).collect();
    }
    // Largest-remainder rounding so segments sum exactly to bar_h.
    let bar_h_f = bar_h as f64;
    let mut shares: Vec<(String, u16, f64)> = roots
        .iter()
        .map(|(n, s)| {
            let raw = (s / total) * bar_h_f;
            let floored = raw.floor();
            (n.clone(), floored as u16, raw - floored)
        })
        .collect();
    let allocated: u16 = shares.iter().map(|(_, h, _)| *h).sum();
    let mut leftover = bar_h.saturating_sub(allocated);
    if leftover > 0 {
        // Stable sort by remainder desc — preserves original order on ties.
        let mut indices: Vec<usize> = (0..shares.len()).collect();
        indices.sort_by(|a, b| {
            shares[*b]
                .2
                .partial_cmp(&shares[*a].2)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for &idx in &indices {
            if leftover == 0 {
                break;
            }
            shares[idx].1 += 1;
            leftover -= 1;
        }
    }
    shares.into_iter().map(|(n, h, _)| (n, h)).collect()
}

fn paint_centered(area: Rect, buf: &mut Buffer, theme: &Theme, text: &str) {
    let style = theme.dim_style();
    let x = area.x + area.width.saturating_sub(text.len() as u16) / 2;
    let y = area.y + area.height / 2;
    write_str(buf, x, y, text, style);
}

fn write_str(buf: &mut Buffer, x: u16, y: u16, s: &str, style: Style) {
    if y >= buf.area.y + buf.area.height || x >= buf.area.x + buf.area.width {
        return;
    }
    buf.set_string(x, y, s, style);
}

fn ceil_hour(secs: f64) -> f64 {
    if secs <= 0.0 {
        return 0.0;
    }
    let hours = (secs / 3600.0).ceil();
    hours * 3600.0
}

/// Tiny `Widget` shim so callers can `f.render_widget(StackedBars{..}, area)`
/// without touching the buffer themselves. Kept as a value so it can hold
/// borrowed inputs cheaply.
pub struct StackedBars<'a> {
    pub theme: &'a Theme,
    pub days: Option<&'a [WeekDayBuckets]>,
    pub in_flight: bool,
}

impl<'a> Widget for StackedBars<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        render(area, buf, self.theme, self.days, self.in_flight);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use chrono::{NaiveDate, Weekday};
    use ratatui::buffer::Buffer;

    #[test]
    fn largest_remainder_rounding_sums_to_bar_height() {
        let roots = vec![
            ("Work".to_string(), 3300.0),    // 55%
            ("Browsing".to_string(), 1800.0), // 30%
            ("Comms".to_string(), 900.0),     // 15%
        ];
        let total = 3300.0 + 1800.0 + 900.0;
        for bar_h in [1u16, 3, 5, 8, 13] {
            let segs = allocate_segments(&roots, total, bar_h);
            let sum: u16 = segs.iter().map(|(_, h)| *h).sum();
            assert_eq!(
                sum, bar_h,
                "segments must sum to bar_h={bar_h}, got {sum}: {segs:?}"
            );
            // Order is preserved.
            assert_eq!(
                segs.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
                vec!["Work", "Browsing", "Comms"]
            );
        }
    }

    #[test]
    fn allocate_segments_handles_zero_inputs() {
        let segs = allocate_segments(&[], 100.0, 5);
        assert!(segs.is_empty());

        let roots = vec![("Work".to_string(), 0.0)];
        let segs = allocate_segments(&roots, 0.0, 5);
        assert_eq!(segs, vec![("Work".to_string(), 0)]);
    }

    #[test]
    fn column_with_zero_total_paints_baseline_dot() {
        let theme = Theme::from_env_pair(Some("truecolor"), None);
        let area = Rect {
            x: 0,
            y: 0,
            width: 30,
            height: 8,
        };
        let mut buf = Buffer::empty(area);
        let day = WeekDayBuckets {
            date: NaiveDate::from_ymd_opt(2026, 5, 4).unwrap(),
            weekday: Weekday::Mon,
            is_future: false,
            roots: Vec::new(),
            total_active_secs: 0.0,
        };
        // Position the column directly so we don't depend on render's
        // layout math.
        paint_column(area, &mut buf, &theme, &day, 1.0, 2);
        // Bottom row should contain the dot glyph somewhere.
        let bottom_y = area.height - 1;
        let mut saw_dot = false;
        for x in 0..area.width {
            if buf[(x, bottom_y)].symbol() == DOT {
                saw_dot = true;
                break;
            }
        }
        assert!(saw_dot, "zero-total past day must paint a baseline dot");
    }

    #[test]
    fn future_column_renders_axis_label_only() {
        let theme = Theme::from_env_pair(Some("truecolor"), None);
        let area = Rect {
            x: 0,
            y: 0,
            width: 30,
            height: 8,
        };
        let mut buf = Buffer::empty(area);
        let day = WeekDayBuckets {
            date: NaiveDate::from_ymd_opt(2026, 5, 8).unwrap(),
            weekday: Weekday::Fri,
            is_future: true,
            roots: Vec::new(),
            total_active_secs: 0.0,
        };
        paint_column(area, &mut buf, &theme, &day, 3600.0, 2);
        // No bar glyphs should be painted into the chart area for a
        // future day. The x-axis label is painted by the parent renderer
        // (paint_x_axis), not paint_column.
        for y in 0..area.height {
            for x in 0..area.width {
                assert_ne!(
                    buf[(x, y)].symbol(),
                    BLOCK,
                    "future column must not paint bar blocks"
                );
                assert_ne!(
                    buf[(x, y)].symbol(),
                    DOT,
                    "future column must not paint baseline dots"
                );
            }
        }
    }
}
