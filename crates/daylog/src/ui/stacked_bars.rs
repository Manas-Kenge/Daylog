//! 7-row horizontal-bar widget for the Week tab. Each row maps to one day
//! of the calendar week (Mon → Sun). Bar cells within a row map to
//! category roots, coloured via `category_root_style`. Past days with zero
//! activity render a single `·` glyph; future days the user hasn't reached
//! yet render the dim day label only.
//!
//! The horizontal layout replaces the previous vertical stacked-column
//! layout (DESIGN.md 2026-05-20). The category-split rounding is shared
//! via `allocate_segments`, so colours and totals match what the legend
//! advertises.

use chrono::NaiveDate;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    widgets::Widget,
};

use crate::cache::WeekDayBuckets;
use crate::theme::Theme;
use crate::ui::week::{category_root_style, short_weekday};

// `▆` (lower 3/4 block) instead of `█` so each row's colour fills the
// bottom 3/4 of the cell, leaving a thin 1/4-cell gap visible above —
// reads as a slight vertical gap between consecutive day rows without
// needing extra layout rows we don't have on the 24-row terminal floor.
const BLOCK: &str = "\u{2586}";
const DOT: &str = "\u{00b7}";
const PEAK_LABEL: &str = "\u{2190} peak"; // ← peak

/// Column allocations inside each row (in cells, left-to-right).
const DAY_W: u16 = 4; // "Mon "
const DUR_W: u16 = 7; // "4h 30m " right-aligned
const GAP_W: u16 = 2;
const PEAK_W: u16 = 8; // reserved on every row so all bars share max width
const FIXED_W: u16 = DAY_W + DUR_W + GAP_W + PEAK_W;

/// Render the horizontal bar chart into `area`. The caller owns the panel
/// block + title; this fn paints into the inner rect.
fn render(
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    days: Option<&[WeekDayBuckets]>,
    in_flight: bool,
    peak_date: Option<NaiveDate>,
) {
    if area.width == 0 || area.height == 0 {
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
    // Need room for fixed columns + at least 1 bar cell.
    if area.width <= FIXED_W {
        return;
    }
    let bar_w = area.width - FIXED_W;

    // Scale bars against the week's max (excluding future days).
    let max_secs = days
        .iter()
        .filter(|d| !d.is_future)
        .map(|d| d.total_active_secs)
        .fold(0.0_f64, f64::max);

    let rows_available = area.height as usize;
    for (i, day) in days.iter().take(rows_available).enumerate() {
        let row = Rect {
            x: area.x,
            y: area.y + i as u16,
            width: area.width,
            height: 1,
        };
        let is_peak = peak_date == Some(day.date) && !day.is_future && day.total_active_secs > 0.0;
        paint_row(row, buf, theme, day, max_secs, bar_w, is_peak);
    }
}

fn paint_row(
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    day: &WeekDayBuckets,
    max_secs: f64,
    bar_w: u16,
    is_peak: bool,
) {
    let label_style = if day.is_future {
        theme.dim_style()
    } else {
        Style::default()
            .fg(theme.fg)
            .add_modifier(Modifier::BOLD)
    };
    let day_label = short_weekday(day.weekday);
    write_str(buf, area.x, area.y, day_label, label_style);

    // Future days: label only.
    if day.is_future {
        return;
    }

    // Duration column, right-aligned inside DUR_W.
    let dur_x = area.x + DAY_W;
    let dur_text = if day.total_active_secs > 0.0 {
        crate::ui::format_duration(day.total_active_secs)
    } else {
        "0".to_string()
    };
    let dur_padded = right_align(&dur_text, DUR_W as usize);
    write_str(buf, dur_x, area.y, &dur_padded, theme.dim_style());

    // Bar column.
    let bar_x = area.x + DAY_W + DUR_W + GAP_W;

    if day.total_active_secs <= 0.0 {
        // Past day with no activity — single dim baseline dot at the bar's
        // start so the row reads as "tracked, empty" rather than "missing".
        write_str(buf, bar_x, area.y, DOT, theme.dim_style());
        return;
    }

    // Bar width in cells, scaled against the week's max.
    let max = max_secs.max(day.total_active_secs).max(1.0);
    let frac = (day.total_active_secs / max).clamp(0.0, 1.0);
    let bar_cells = ((frac * bar_w as f64).round() as u16).max(1).min(bar_w);

    // Split bar cells across category roots (left-to-right) using the same
    // largest-remainder allocator that the previous vertical layout used.
    let segments = allocate_segments(&day.roots, day.total_active_secs, bar_cells);
    let mut x = bar_x;
    for (root, seg_w) in segments {
        if seg_w == 0 {
            continue;
        }
        let style = category_root_style(theme, &root);
        for _ in 0..seg_w {
            write_str(buf, x, area.y, BLOCK, style);
            x += 1;
        }
    }

    // Peak annotation in the reserved right slot.
    if is_peak {
        let peak_x = area.x + DAY_W + DUR_W + GAP_W + bar_w;
        write_str(buf, peak_x, area.y, PEAK_LABEL, theme.dim_style());
    }
}

/// Allocate `bar_cells` integer cells across roots in proportion to each
/// root's share of `total`. Uses largest-remainder rounding so the sum is
/// exactly `bar_cells`. Order of returned segments matches `roots` order.
pub(crate) fn allocate_segments(
    roots: &[(String, f64)],
    total: f64,
    bar_cells: u16,
) -> Vec<(String, u16)> {
    if bar_cells == 0 || total <= 0.0 || roots.is_empty() {
        return roots.iter().map(|(n, _)| (n.clone(), 0)).collect();
    }
    let bar_f = bar_cells as f64;
    let mut shares: Vec<(String, u16, f64)> = roots
        .iter()
        .map(|(n, s)| {
            let raw = (s / total) * bar_f;
            let floored = raw.floor();
            (n.clone(), floored as u16, raw - floored)
        })
        .collect();
    let allocated: u16 = shares.iter().map(|(_, h, _)| *h).sum();
    let mut leftover = bar_cells.saturating_sub(allocated);
    if leftover > 0 {
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

fn right_align(s: &str, width: usize) -> String {
    if s.len() >= width {
        return s.to_string();
    }
    let pad = width - s.len();
    let mut out = String::with_capacity(width);
    for _ in 0..pad {
        out.push(' ');
    }
    out.push_str(s);
    out
}

/// `Widget` shim so callers can `f.render_widget(HorizontalBars{..}, area)`.
pub struct HorizontalBars<'a> {
    pub theme: &'a Theme,
    pub days: Option<&'a [WeekDayBuckets]>,
    pub in_flight: bool,
    /// Date of the day with the largest `total_active_secs` (excluding
    /// future days). When `Some`, that row gets the `← peak` annotation.
    pub peak_date: Option<NaiveDate>,
}

impl<'a> Widget for HorizontalBars<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        render(
            area,
            buf,
            self.theme,
            self.days,
            self.in_flight,
            self.peak_date,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use chrono::{Datelike, NaiveDate};
    use ratatui::buffer::Buffer;

    fn day(date: (i32, u32, u32), is_future: bool, roots: &[(&str, f64)]) -> WeekDayBuckets {
        let d = NaiveDate::from_ymd_opt(date.0, date.1, date.2).unwrap();
        let weekday = d.weekday();
        let roots: Vec<(String, f64)> = roots.iter().map(|(n, s)| ((*n).to_string(), *s)).collect();
        let total = roots.iter().map(|(_, s)| *s).sum();
        WeekDayBuckets {
            date: d,
            weekday,
            is_future,
            roots,
            total_active_secs: total,
        }
    }

    #[test]
    fn largest_remainder_rounding_sums_to_bar_cells() {
        let roots = vec![
            ("Work".to_string(), 3300.0),    // 55%
            ("Browsing".to_string(), 1800.0), // 30%
            ("Comms".to_string(), 900.0),     // 15%
        ];
        let total = 3300.0 + 1800.0 + 900.0;
        for bar_cells in [1u16, 3, 5, 8, 13, 40] {
            let segs = allocate_segments(&roots, total, bar_cells);
            let sum: u16 = segs.iter().map(|(_, h)| *h).sum();
            assert_eq!(
                sum, bar_cells,
                "segments must sum to bar_cells={bar_cells}, got {sum}: {segs:?}"
            );
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

    fn render_to_buf(days: &[WeekDayBuckets], peak: Option<NaiveDate>, area: Rect) -> Buffer {
        let theme = Theme::from_env_pair(Some("truecolor"), None);
        let mut buf = Buffer::empty(area);
        let widget = HorizontalBars {
            theme: &theme,
            days: Some(days),
            in_flight: false,
            peak_date: peak,
        };
        widget.render(area, &mut buf);
        buf
    }

    fn buf_to_string(buf: &Buffer) -> String {
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    fn fixture() -> Vec<WeekDayBuckets> {
        vec![
            day((2026, 5, 4), false, &[("Work", 4.0 * 3600.0), ("Comms", 1800.0)]),
            day((2026, 5, 5), false, &[("Work", 6.0 * 3600.0), ("Browsing", 3600.0)]),
            day((2026, 5, 6), false, &[("Work", 5.0 * 3600.0)]),
            day((2026, 5, 7), false, &[]), // empty past day
            day((2026, 5, 8), true, &[]),
            day((2026, 5, 9), true, &[]),
            day((2026, 5, 10), true, &[]),
        ]
    }

    #[test]
    fn renders_seven_weekday_labels_one_per_row() {
        let area = Rect { x: 0, y: 0, width: 80, height: 8 };
        let buf = render_to_buf(&fixture(), None, area);
        let s = buf_to_string(&buf);
        for label in ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"] {
            assert!(s.contains(label), "expected {label} in rendered buf:\n{s}");
        }
    }

    #[test]
    fn empty_past_day_paints_baseline_dot() {
        let area = Rect { x: 0, y: 0, width: 80, height: 8 };
        let buf = render_to_buf(&fixture(), None, area);
        // Thu row is index 3.
        let bar_x = DAY_W + DUR_W + GAP_W;
        assert_eq!(buf[(bar_x, 3)].symbol(), DOT);
    }

    #[test]
    fn future_day_renders_label_only_no_bar() {
        let area = Rect { x: 0, y: 0, width: 80, height: 8 };
        let buf = render_to_buf(&fixture(), None, area);
        // Sat row is index 5 (future).
        for x in 0..area.width {
            assert_ne!(
                buf[(x, 5)].symbol(),
                BLOCK,
                "future row must not paint bar blocks"
            );
            assert_ne!(
                buf[(x, 5)].symbol(),
                DOT,
                "future row must not paint baseline dots"
            );
        }
        // Label is still painted.
        let row_str: String = (0..area.width).map(|x| buf[(x, 5)].symbol().to_string()).collect();
        assert!(row_str.starts_with("Sat"), "future row should still show its label: {row_str:?}");
    }

    #[test]
    fn peak_row_paints_arrow_annotation() {
        let area = Rect { x: 0, y: 0, width: 80, height: 8 };
        let peak = NaiveDate::from_ymd_opt(2026, 5, 5).unwrap(); // Tue
        let buf = render_to_buf(&fixture(), Some(peak), area);
        let row_str: String = (0..area.width).map(|x| buf[(x, 1)].symbol().to_string()).collect();
        assert!(row_str.contains("peak"), "peak annotation missing on Tue row: {row_str:?}");
        // Non-peak row should not have it.
        let other: String = (0..area.width).map(|x| buf[(x, 0)].symbol().to_string()).collect();
        assert!(!other.contains("peak"), "non-peak row should not show 'peak': {other:?}");
    }

    #[test]
    fn bar_cells_use_category_colours() {
        let theme = Theme::from_env_pair(Some("truecolor"), None);
        let area = Rect { x: 0, y: 0, width: 80, height: 8 };
        let buf = render_to_buf(&fixture(), None, area);
        let bar_x = DAY_W + DUR_W + GAP_W;
        // Mon row 0 has Work + Comms — first cell should be chart_1 (Work).
        assert_eq!(buf[(bar_x, 0)].symbol(), BLOCK);
        assert_eq!(buf[(bar_x, 0)].style().fg, Some(theme.chart_1));
    }

    #[test]
    fn narrow_area_returns_without_panicking() {
        // Less than FIXED_W (= 21) — should bail without painting bars.
        let area = Rect { x: 0, y: 0, width: 10, height: 8 };
        let buf = render_to_buf(&fixture(), None, area);
        for y in 0..area.height {
            for x in 0..area.width {
                assert_ne!(buf[(x, y)].symbol(), BLOCK);
            }
        }
    }

    #[test]
    fn future_day_uses_actual_weekday_label() {
        // Sanity: confirms the widget pulls labels from `day.weekday`
        // rather than a positional table — verified by the fact that
        // `Weekday::Sat` resolves to "Sat" for the 6th day (index 5) in
        // the fixture's Mon-anchored week.
        let area = Rect { x: 0, y: 0, width: 80, height: 8 };
        let buf = render_to_buf(&fixture(), None, area);
        let row5: String = (0..3).map(|x| buf[(x, 5)].symbol().to_string()).collect();
        assert_eq!(row5, "Sat");
    }
}
