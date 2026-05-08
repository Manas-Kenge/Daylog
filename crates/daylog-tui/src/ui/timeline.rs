//! 24h timeline heatmap — 96 cells × 15-min slots, dominant category per
//! slot. Mirrors the desktop `Timeline.tsx` widget. The bucketize96
//! algorithm is a Rust port of `src/lib/timeline.ts`; both surfaces must
//! agree on which slot each event lands in.

use std::collections::HashMap;

use chrono::{Local, Timelike};
use daylog_core::aggregate::CategorizedEvent;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::theme::Theme;

const SLOT_SECS: f64 = 15.0 * 60.0;
const TOTAL_SLOTS: usize = 96;

/// One 15-minute slot in today's timeline. `category` is the dominant
/// (longest-contributing) root in this window, or None for empty slots.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineSlot {
    pub index: usize,
    pub category: Option<String>,
    pub duration_secs: f64,
}

/// Place each event into the 15-min slots it touches; the dominant
/// category per slot wins. Mirrors the desktop's `bucketize96` exactly so
/// the two surfaces can't disagree on what was happening at 14:30.
///
/// Local time-of-day is computed via `chrono::Local`, matching the JS
/// `setHours(0,0,0,0)` floor that the desktop uses to compute slot index.
pub fn bucketize96(events: &[CategorizedEvent]) -> Vec<TimelineSlot> {
    let mut slots: Vec<TimelineSlot> = (0..TOTAL_SLOTS)
        .map(|i| TimelineSlot {
            index: i,
            category: None,
            duration_secs: 0.0,
        })
        .collect();

    let mut tallies: Vec<HashMap<String, f64>> =
        (0..TOTAL_SLOTS).map(|_| HashMap::new()).collect();

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
        // Safety cap mirrors the desktop's `safety < 200` — events
        // longer than ~50h shouldn't appear, but the cap keeps a
        // malformed event from looping forever.
        for _ in 0..200 {
            if remaining <= 0.0 {
                break;
            }
            let slot_idx_f = (cursor / SLOT_SECS).floor();
            if slot_idx_f < 0.0 {
                break;
            }
            let slot_idx = slot_idx_f as usize;
            if slot_idx >= TOTAL_SLOTS {
                break;
            }
            let next_boundary = ((slot_idx + 1) as f64) * SLOT_SECS;
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

/// Render the 24h timeline panel. Four rows: top border, 96-cell strip,
/// axis tick row (`00       06       12       18       23`), bottom
/// border. The bold title sits on the top border so the section reads as
/// a header, not as part of the dim border chrome.
pub fn render(
    f: &mut Frame,
    area: Rect,
    theme: &Theme,
    events: Option<&Vec<CategorizedEvent>>,
    in_flight: bool,
) {
    let title_style = Style::default().fg(theme.fg).add_modifier(Modifier::BOLD);
    let title = if in_flight {
        Line::from(vec![
            Span::styled(" Today's timeline ", title_style),
            Span::styled("\u{21bb} ", Style::default().fg(theme.dim)),
        ])
    } else {
        Line::from(Span::styled(" Today's timeline ", title_style))
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_dim_style())
        .title(title);

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Inside the 2-row inner area: row 0 = cell strip, row 1 = axis ticks.
    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(inner);
    let cells_area = inner_chunks[0];
    let axis_area = inner_chunks.get(1).copied();

    let Some(events) = events else {
        // Skeleton — single dim ellipsis on the cell row, axis still
        // renders so the panel shape doesn't collapse.
        let p = Paragraph::new("\u{2026}").style(Style::default().fg(theme.dim));
        f.render_widget(p, cells_area);
        if let Some(axis) = axis_area {
            f.render_widget(axis_paragraph(theme, cells_area.width), axis);
        }
        return;
    };

    let slots = bucketize96(events);

    // Cells run flush — no inline dividers. The panel block's borders
    // are the only frame; the axis row below anchors hours via labels.
    let spans: Vec<Span<'static>> = slots
        .iter()
        .map(|slot| match &slot.category {
            Some(cat) => Span::styled(
                "\u{2588}",
                Style::default().fg(theme.category_color(cat)),
            ),
            None => Span::styled("\u{00b7}", Style::default().fg(theme.border_dim)),
        })
        .collect();

    let p = Paragraph::new(Line::from(spans));
    f.render_widget(p, cells_area);

    if let Some(axis) = axis_area {
        f.render_widget(axis_paragraph(theme, cells_area.width), axis);
    }
}

/// Build the axis row: hour labels positioned under the cell strip at
/// 00 / 06 / 12 / 18 / 23. With flush cells (no dividers), hour `h`
/// starts at column `h * 4` — 4 slots per hour, 96 slots total.
fn axis_paragraph(theme: &Theme, cell_width: u16) -> Paragraph<'static> {
    let width = (cell_width as usize).max(1);
    let labels = [(0_usize, "00"), (6, "06"), (12, "12"), (18, "18"), (23, "23")];
    let mut row = vec![' '; width];
    for (h, label) in labels {
        let col = h * 4;
        for (i, ch) in label.chars().enumerate() {
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
    fn bucketize96_emits_96_slots() {
        let slots = bucketize96(&[]);
        assert_eq!(slots.len(), 96);
        assert!(slots.iter().all(|s| s.category.is_none()));
        assert!(slots.iter().all(|s| s.duration_secs == 0.0));
    }

    #[test]
    fn bucketize96_dominant_category_wins_per_slot() {
        // Two events targeting the same slot; the longer-duration one
        // takes the slot. Use the local-time hour computed from a UTC
        // event timestamp so the test is timezone-invariant.
        let events = vec![
            ev(12, 5, 600.0, &["Browsing"]), // 10 min Browsing
            ev(12, 5, 60.0, &["Programming"]), // 1 min Programming
        ];
        let slots = bucketize96(&events);
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
    fn bucketize96_uncategorized_for_empty_path() {
        let events = vec![ev(8, 0, 600.0, &[])];
        let slots = bucketize96(&events);
        assert!(slots.iter().any(|s| s.category.as_deref() == Some("Uncategorized")));
    }

    #[test]
    fn bucketize96_skips_zero_or_negative_durations() {
        let events = vec![
            ev(10, 0, 0.0, &["Browsing"]),
            ev(11, 0, -100.0, &["Programming"]),
        ];
        let slots = bucketize96(&events);
        assert!(slots.iter().all(|s| s.category.is_none()));
    }
}
