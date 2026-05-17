//! 24h timeline barcode. N = inner panel width; each cell is `▌` so
//! right halves render as gaps. Borderless — the section header lives in
//! the parent (overview::render_timeline_section). This module just paints
//! the barcode stripe + hour-axis row into the area it's given.

use std::collections::HashMap;

use chrono::{Local, Timelike};
use crate::data::aggregate::CategorizedEvent;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use throbber_widgets_tui::ThrobberState;

use crate::theme::Theme;
use crate::ui::render_skeleton_body;

const SECS_PER_DAY: f64 = 24.0 * 60.0 * 60.0;

/// One time slot in today's timeline. `category` is the dominant
/// (longest-contributing) root in this window, or None for empty slots.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineSlot {
    pub index: usize,
    pub category: Option<String>,
    pub duration_secs: f64,
}

/// Place each event into the `n` slots it touches; dominant root wins
/// per slot. Slot index uses local time-of-day.
pub fn bucketize_n(events: &[CategorizedEvent], n: usize) -> Vec<TimelineSlot> {
    if n == 0 {
        return Vec::new();
    }
    let slot_secs = SECS_PER_DAY / n as f64;

    let mut slots: Vec<TimelineSlot> = (0..n)
        .map(|i| TimelineSlot {
            index: i,
            category: None,
            duration_secs: 0.0,
        })
        .collect();

    let mut tallies: Vec<HashMap<String, f64>> = (0..n).map(|_| HashMap::new()).collect();

    for ev in events {
        // Local time-of-day, in seconds since midnight.
        let local = ev.timestamp.with_timezone(&Local);
        let from_day_start =
            (local.hour() as f64) * 3600.0 + (local.minute() as f64) * 60.0 + local.second() as f64;
        if from_day_start < 0.0 || ev.duration <= 0.0 {
            continue;
        }
        let cat = category_root(&ev.category);

        let mut remaining = ev.duration;
        let mut cursor = from_day_start;
        // Safety cap: malformed event mustn't loop forever.
        let cap = (n * 2).max(200);
        for _ in 0..cap {
            if remaining <= 0.0 {
                break;
            }
            let slot_idx_f = (cursor / slot_secs).floor();
            if slot_idx_f < 0.0 {
                break;
            }
            let slot_idx = slot_idx_f as usize;
            if slot_idx >= n {
                break;
            }
            let next_boundary = ((slot_idx + 1) as f64) * slot_secs;
            let chunk = remaining.min(next_boundary - cursor);
            let entry = tallies[slot_idx].entry(cat.clone()).or_insert(0.0);
            *entry += chunk;
            remaining -= chunk;
            cursor = next_boundary;
        }
    }

    for (idx, tally) in tallies.into_iter().enumerate() {
        let mut best = String::new();
        let mut best_val = 0.0_f64;
        let mut total = 0.0_f64;
        for (k, v) in tally {
            total += v;
            if v > best_val {
                best_val = v;
                best = k;
            }
        }
        slots[idx].duration_secs = total;
        slots[idx].category = if best.is_empty() { None } else { Some(best) };
    }

    slots
}

fn category_root(name: &[String]) -> String {
    name.first()
        .cloned()
        .unwrap_or_else(|| "Uncategorized".to_string())
}

/// Barcode + hour-axis row. Caller renders the section header above this area.
pub fn render(
    f: &mut Frame,
    area: Rect,
    theme: &Theme,
    events: Option<&Vec<CategorizedEvent>>,
    in_flight: bool,
    throbber: &ThrobberState,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let inner = area;

    let (stripes_area, axis_area) = if inner.height >= 2 {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(inner);
        (chunks[0], Some(chunks[1]))
    } else {
        (inner, None)
    };

    let Some(events) = events else {
        // Render axis even in skeleton — panel shape stays stable.
        render_skeleton_body(f, stripes_area, theme, throbber, in_flight);
        if let Some(axis) = axis_area {
            f.render_widget(axis_paragraph(theme, stripes_area.width), axis);
        }
        return;
    };

    let n = stripes_area.width as usize;
    let slots = bucketize_n(events, n);

    let line: Line<'static> = Line::from(
        slots
            .iter()
            .map(|slot| match &slot.category {
                Some(cat) => Span::styled(
                    "\u{258C}",
                    Style::default().fg(theme.category_color(cat)),
                ),
                None => Span::raw(" "),
            })
            .collect::<Vec<_>>(),
    );

    let lines: Vec<Line<'static>> = (0..stripes_area.height as usize)
        .map(|_| line.clone())
        .collect();
    let p = Paragraph::new(lines);
    f.render_widget(p, stripes_area);

    if let Some(axis) = axis_area {
        f.render_widget(axis_paragraph(theme, stripes_area.width), axis);
    }
}

/// Hour-tick row positioned by hour-fraction of stripe width.
fn axis_paragraph(theme: &Theme, width: u16) -> Paragraph<'static> {
    let width = (width as usize).max(1);
    let labels = [(0_usize, "00"), (6, "06"), (12, "12"), (18, "18"), (23, "23")];
    let mut row = vec![' '; width];
    for (h, label) in labels {
        let col = ((h as f64 / 24.0) * width as f64).round() as usize;
        for (i, ch) in label.chars().enumerate() {
            // Right-anchor "23" so it doesn't push past the row end on small widths.
            let target = if h == 23 {
                width.saturating_sub(label.len()) + i
            } else {
                col + i
            };
            if let Some(cell) = row.get_mut(target) {
                *cell = ch;
            }
        }
    }
    let text: String = row.into_iter().collect();
    Paragraph::new(Line::from(Span::styled(
        text,
        Style::default().fg(theme.dim),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use serde_json::Value;

    fn ev(hour: u32, minute: u32, dur_secs: f64, category: &[&str]) -> CategorizedEvent {
        // Build at UTC; bucketize will read with Local. For deterministic
        // tests we ground at UTC noon-ish so DST adjustments don't push
        // us past a slot boundary on most machines.
        let ts = Utc
            .with_ymd_and_hms(2026, 5, 8, hour, minute, 0)
            .single()
            .expect("valid timestamp");
        CategorizedEvent {
            timestamp: ts,
            duration: dur_secs,
            data: Value::Null,
            category: category.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    #[test]
    fn bucketize_n_emits_96_slots() {
        let slots = bucketize_n(&[], 96);
        assert_eq!(slots.len(), 96);
        assert!(slots.iter().all(|s| s.category.is_none()));
        assert!(slots.iter().all(|s| s.duration_secs == 0.0));
    }

    #[test]
    fn bucketize_n_dominant_category_wins_per_slot() {
        // Longer-duration event wins. UTC ts → local hour keeps it tz-invariant.
        let events = vec![
            ev(12, 5, 600.0, &["Browsing"]), // 10 min Browsing
            ev(12, 5, 60.0, &["Programming"]), // 1 min Programming
        ];
        let slots = bucketize_n(&events, 96);
        let occupied: Vec<&TimelineSlot> = slots.iter().filter(|s| s.category.is_some()).collect();
        assert!(!occupied.is_empty(), "events should populate at least one slot");
        let dominant = &occupied[0].category;
        assert_eq!(
            dominant.as_deref(),
            Some("Browsing"),
            "longer event should win the slot"
        );
    }

    #[test]
    fn bucketize_n_uncategorized_for_empty_path() {
        let events = vec![ev(8, 0, 600.0, &[])];
        let slots = bucketize_n(&events, 96);
        assert!(slots.iter().any(|s| s.category.as_deref() == Some("Uncategorized")));
    }

    #[test]
    fn bucketize_n_skips_zero_or_negative_durations() {
        let events = vec![
            ev(10, 0, 0.0, &["Browsing"]),
            ev(11, 0, -100.0, &["Programming"]),
        ];
        let slots = bucketize_n(&events, 96);
        assert!(slots.iter().all(|s| s.category.is_none()));
    }

    #[test]
    fn bucketize_n_handles_arbitrary_widths() {
        // Width-adaptive: the slot count must match `n`, including the
        // edge case n == 0 (returns empty so callers can early-return).
        assert_eq!(bucketize_n(&[], 50).len(), 50);
        assert_eq!(bucketize_n(&[], 200).len(), 200);
        assert!(bucketize_n(&[], 0).is_empty());
    }
}
