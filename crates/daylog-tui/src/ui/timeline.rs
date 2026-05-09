//! 24h timeline barcode — N width-adaptive cells, dominant category per
//! slot. Each cell renders `▌` (LEFT HALF BLOCK) so the right half stays
//! at panel background, producing visible gaps between adjacent stripes.
//! N is set to the inner panel width at render time, so the bar always
//! fills its rect at the densest resolution the terminal supports.

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

const SECS_PER_DAY: f64 = 24.0 * 60.0 * 60.0;

/// One time slot in today's timeline. `category` is the dominant
/// (longest-contributing) root in this window, or None for empty slots.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineSlot {
    pub index: usize,
    pub category: Option<String>,
    pub duration_secs: f64,
}

/// Place each event into the `n` time slots it touches; the dominant
/// category per slot wins. Generalizes the desktop's `bucketize96` to an
/// arbitrary slot count so the TUI can scale resolution to terminal width.
///
/// Local time-of-day is computed via `chrono::Local`, matching the JS
/// `setHours(0,0,0,0)` floor that the desktop uses to compute slot index.
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
        // Safety cap mirrors the desktop's `safety < 200` — events
        // longer than ~50h shouldn't appear, but the cap keeps a
        // malformed event from looping forever. Scaled with n so very
        // dense bars still terminate quickly.
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

/// Render the 24h timeline panel as a barcode with an axis row. Six
/// rows: top border, 3 stripe rows, axis (`00 06 12 18 23`), bottom
/// border. Each stripe row repeats the same line of `▌` half-blocks
/// colored by category — adjacent cells alternate color/background,
/// creating the gap effect without burning extra cells. The axis is
/// scaled to the dynamic stripe count so labels stay aligned to hours.
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

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    // Reserve the bottom row for the axis when there's room for both
    // stripes and a label row. On a single inner row, just paint stripes.
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
        // Skeleton — dim ellipsis on the stripes; axis still renders so
        // the panel shape doesn't collapse before data lands.
        let p = Paragraph::new("\u{2026}").style(Style::default().fg(theme.dim));
        f.render_widget(p, stripes_area);
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

/// Hour-tick row: `00 / 06 / 12 / 18 / 23` positioned by hour-fraction
/// of the stripe width. Generalizes the old fixed `h * 4` anchor (which
/// only worked at width=96) to any dynamic stripe count.
fn axis_paragraph(theme: &Theme, width: u16) -> Paragraph<'static> {
    let width = (width as usize).max(1);
    let labels = [(0_usize, "00"), (6, "06"), (12, "12"), (18, "18"), (23, "23")];
    let mut row = vec![' '; width];
    for (h, label) in labels {
        let col = ((h as f64 / 24.0) * width as f64).round() as usize;
        for (i, ch) in label.chars().enumerate() {
            // Right-anchor the trailing "23" so it doesn't push past the
            // end of the row at small widths.
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
        // Two events targeting the same slot; the longer-duration one
        // takes the slot. Use the local-time hour computed from a UTC
        // event timestamp so the test is timezone-invariant.
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
