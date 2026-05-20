//! In-process ports of aw-transform/* AQL primitives.

use chrono::Duration as ChronoDuration;
use fancy_regex::Regex as FancyRegex;
use serde_json::{Map, Value};
use url::Url;

use crate::data::aw_client::Event;

fn endtime(e: &Event) -> chrono::DateTime<chrono::Utc> {
    e.timestamp + ChronoDuration::nanoseconds((e.duration * 1_000_000_000.0) as i64)
}

fn duration_secs(d: ChronoDuration) -> f64 {
    let nanos = d.num_nanoseconds().unwrap_or_else(|| d.num_milliseconds() * 1_000_000);
    nanos as f64 / 1_000_000_000.0
}

pub fn sort_by_timestamp(mut events: Vec<Event>) -> Vec<Event> {
    events.sort_by_key(|e| e.timestamp);
    events
}

pub fn sort_by_duration(mut events: Vec<Event>) -> Vec<Event> {
    events.sort_by(|a, b| b.duration.partial_cmp(&a.duration).unwrap_or(std::cmp::Ordering::Equal));
    events
}

pub fn filter_keyvals(events: Vec<Event>, key: &str, values: &[Value]) -> Vec<Event> {
    events
        .into_iter()
        .filter(|e| match e.data.get(key) {
            Some(v) => values.iter().any(|allowed| allowed == v),
            None => false,
        })
        .collect()
}

pub fn merge_events_by_keys(events: Vec<Event>, keys: &[&str]) -> Vec<Event> {
    use std::collections::HashMap;
    let mut map: HashMap<Vec<Value>, Event> = HashMap::new();
    for ev in events {
        // Skip events missing any of the requested keys (matches upstream).
        let mut composite: Vec<Value> = Vec::with_capacity(keys.len());
        let mut skip = false;
        for k in keys {
            match ev.data.get(*k) {
                Some(v) => composite.push(v.clone()),
                None => {
                    skip = true;
                    break;
                }
            }
        }
        if skip {
            continue;
        }
        map.entry(composite)
            .and_modify(|merged| {
                merged.duration += ev.duration;
            })
            .or_insert_with(|| {
                let mut data = Map::new();
                for k in keys {
                    if let Some(v) = ev.data.get(*k) {
                        data.insert((*k).to_string(), v.clone());
                    }
                }
                Event {
                    id: None,
                    timestamp: ev.timestamp,
                    duration: ev.duration,
                    data: Value::Object(data),
                }
            });
    }
    map.into_values().collect()
}

pub fn filter_period_intersect(events: Vec<Event>, filter_events: Vec<Event>) -> Vec<Event> {
    let events_sorted = sort_by_timestamp(events);
    let filter_sorted = sort_by_timestamp(filter_events);
    let mut out: Vec<Event> = Vec::new();
    let mut cursor = 0usize;
    for ev in &events_sorted {
        let ev_end = endtime(ev);
        while cursor < filter_sorted.len() && endtime(&filter_sorted[cursor]) <= ev.timestamp {
            cursor += 1;
        }
        let mut i = cursor;
        while i < filter_sorted.len() {
            let fe = &filter_sorted[i];
            if fe.timestamp >= ev_end {
                break;
            }
            let fe_end = endtime(fe);
            let new_start = std::cmp::max(ev.timestamp, fe.timestamp);
            let new_end = std::cmp::min(ev_end, fe_end);
            if new_end > new_start {
                out.push(Event {
                    id: ev.id,
                    timestamp: new_start,
                    duration: duration_secs(new_end - new_start),
                    data: ev.data.clone(),
                });
            }
            i += 1;
        }
    }
    out
}

pub fn split_url_events(events: Vec<Event>) -> Vec<Event> {
    events.into_iter().map(split_one_url).collect()
}

fn split_one_url(mut ev: Event) -> Event {
    let Some(url_str) = ev.data.get("url").and_then(|v| v.as_str()).map(str::to_owned) else {
        return ev;
    };
    if let Ok(parsed) = Url::parse(&url_str) {
        if let Value::Object(ref mut m) = ev.data {
            if let Some(host) = parsed.host_str() {
                let trimmed = host.strip_prefix("www.").unwrap_or(host).to_string();
                m.insert("$domain".to_string(), Value::String(trimmed));
            }
            m.insert(
                "$protocol".to_string(),
                Value::String(parsed.scheme().to_string()),
            );
            m.insert("$path".to_string(), Value::String(parsed.path().to_string()));
            if let Some(q) = parsed.query() {
                m.insert("$params".to_string(), Value::String(q.to_string()));
            }
        }
    }
    ev
}

pub struct CompiledRule {
    pub category: Vec<String>,
    pub regex: Option<FancyRegex>,
}

pub fn compile_rules(
    cfg: &crate::data::categories::CategoryConfig,
) -> Result<Vec<CompiledRule>, fancy_regex::Error> {
    let mut out = Vec::with_capacity(cfg.categories.len());
    for cat in &cfg.categories {
        let regex = match &cat.rule {
            crate::data::categories::Rule::Regex { regex, ignore_case } => {
                // fancy_regex has no RegexBuilder API; embed `(?i)` for
                // case-insensitivity, matching upstream's approach.
                let pat = if *ignore_case {
                    format!("(?i){regex}")
                } else {
                    regex.clone()
                };
                Some(FancyRegex::new(&pat)?)
            }
            crate::data::categories::Rule::None => None,
        };
        out.push(CompiledRule {
            category: cat.name.clone(),
            regex,
        });
    }
    Ok(out)
}

pub fn categorize(events: Vec<Event>, rules: &[CompiledRule]) -> Vec<Event> {
    events
        .into_iter()
        .map(|e| categorize_one(e, rules))
        .collect()
}

fn rule_matches(rule: &CompiledRule, ev: &Event) -> bool {
    let Some(re) = rule.regex.as_ref() else {
        return false;
    };
    let Value::Object(ref m) = ev.data else {
        return false;
    };
    for v in m.values() {
        if let Some(s) = v.as_str() {
            if re.is_match(s).unwrap_or(false) {
                return true;
            }
        }
    }
    false
}

fn categorize_one(mut ev: Event, rules: &[CompiledRule]) -> Event {
    let mut chosen: Vec<String> = vec!["Uncategorized".to_string()];
    for rule in rules {
        if rule_matches(rule, &ev) {
            // Deepest category wins; ties prefer the latest match
            // (matches upstream `_pick_highest_ranking_category`).
            if rule.category.len() >= chosen.len() {
                chosen = rule.category.clone();
            }
        }
    }
    if let Value::Object(ref mut m) = ev.data {
        m.insert("$category".to_string(), Value::Array(chosen.into_iter().map(Value::String).collect()));
    }
    ev
}

/// Port of `aw-transform/src/flood.rs`.
pub fn flood(events: Vec<Event>, pulsetime: ChronoDuration) -> Vec<Event> {
    let mut new_events: Vec<Event> = Vec::new();
    let events_sorted = sort_by_timestamp(events);
    let mut iter = events_sorted.into_iter().peekable();

    let mut gap_prev: Option<ChronoDuration> = None;
    let mut retry_e: Option<Event> = None;
    let negative_gap_trim = ChronoDuration::milliseconds(100);

    loop {
        let mut e1 = match retry_e.take() {
            Some(e) => e,
            None => match iter.next() {
                Some(e) => e,
                None => break,
            },
        };

        if let Some(gap) = gap_prev.take() {
            let half = gap / 2;
            e1.timestamp -= half;
            e1.duration += duration_secs(half);
        }

        let e2 = match iter.peek() {
            Some(e) => e.clone(),
            None => {
                new_events.push(e1);
                break;
            }
        };

        let gap = e2.timestamp - endtime(&e1);

        if gap < ChronoDuration::seconds(0) {
            if e1.data == e2.data {
                // Same data + overlap: safe to merge.
                let start = std::cmp::min(e1.timestamp, e2.timestamp);
                let end = std::cmp::max(endtime(&e1), endtime(&e2));
                e1.timestamp = start;
                e1.duration = duration_secs(end - start);
                let _ = iter.next();
                retry_e = Some(e1);
                continue;
            } else if gap < -negative_gap_trim {
                // Differing data + significant overlap: upstream warns and passes through.
            }
        } else if gap < pulsetime {
            if e1.data == e2.data {
                let start = std::cmp::min(e1.timestamp, e2.timestamp);
                let end = std::cmp::max(endtime(&e1), endtime(&e2));
                e1.timestamp = start;
                e1.duration = duration_secs(end - start);
                let _ = iter.next();
                retry_e = Some(e1);
                continue;
            } else {
                e1.duration += duration_secs(gap / 2);
                gap_prev = Some(gap);
            }
        }

        new_events.push(e1);
    }
    new_events
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use serde_json::json;

    fn event(ts: chrono::DateTime<chrono::Utc>, dur_secs: f64, data: Value) -> Event {
        Event {
            id: None,
            timestamp: ts,
            duration: dur_secs,
            data,
        }
    }

    fn t(s: i64) -> chrono::DateTime<chrono::Utc> {
        Utc.timestamp_opt(s, 0).single().unwrap()
    }


    #[test]
    fn sort_by_duration_descending() {
        let a = event(t(0), 1.0, json!({"k":"a"}));
        let b = event(t(0), 5.0, json!({"k":"b"}));
        let c = event(t(0), 3.0, json!({"k":"c"}));
        let got = sort_by_duration(vec![a, b, c]);
        let durs: Vec<f64> = got.iter().map(|e| e.duration).collect();
        assert_eq!(durs, vec![5.0, 3.0, 1.0]);
    }


    #[test]
    fn filter_keyvals_keeps_only_matching_status() {
        let evs = vec![
            event(t(0), 1.0, json!({"status":"afk"})),
            event(t(1), 1.0, json!({"status":"not-afk"})),
            event(t(2), 1.0, json!({"status":"not-afk"})),
        ];
        let got = filter_keyvals(evs, "status", &[json!("not-afk")]);
        assert_eq!(got.len(), 2);
    }

    #[test]
    fn filter_keyvals_drops_events_missing_key() {
        let evs = vec![
            event(t(0), 1.0, json!({"other":"x"})),
            event(t(1), 1.0, json!({"status":"not-afk"})),
        ];
        let got = filter_keyvals(evs, "status", &[json!("not-afk")]);
        assert_eq!(got.len(), 1);
    }


    #[test]
    fn merge_by_app_sums_durations() {
        let evs = vec![
            event(t(0), 10.0, json!({"app":"brave","title":"a"})),
            event(t(5), 30.0, json!({"app":"brave","title":"b"})),
            event(t(10), 5.0, json!({"app":"kitty","title":"x"})),
        ];
        let mut got = merge_events_by_keys(evs, &["app"]);
        got.sort_by(|a, b| b.duration.partial_cmp(&a.duration).unwrap());
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].data.get("app").unwrap(), &json!("brave"));
        assert!((got[0].duration - 40.0).abs() < 1e-6);
        assert_eq!(got[1].data.get("app").unwrap(), &json!("kitty"));
        assert!((got[1].duration - 5.0).abs() < 1e-6);
    }

    #[test]
    fn merge_skips_events_missing_keys() {
        let evs = vec![
            event(t(0), 10.0, json!({"app":"brave"})),
            event(t(1), 5.0, json!({"title":"x"})),
        ];
        let got = merge_events_by_keys(evs, &["app"]);
        assert_eq!(got.len(), 1);
        assert!((got[0].duration - 10.0).abs() < 1e-6);
    }


    #[test]
    fn intersect_clips_to_filter_window() {
        let evs = vec![event(t(0), 100.0, json!({"app":"x"}))];
        let filters = vec![event(t(30), 40.0, json!({"status":"not-afk"}))];
        let got = filter_period_intersect(evs, filters);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].timestamp, t(30));
        assert!((got[0].duration - 40.0).abs() < 1e-6);
    }

    #[test]
    fn intersect_emits_one_slice_per_overlapping_filter() {
        let evs = vec![event(t(0), 100.0, json!({"app":"x"}))];
        let filters = vec![
            event(t(10), 20.0, json!({})),
            event(t(60), 20.0, json!({})),
        ];
        let got = filter_period_intersect(evs, filters);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].timestamp, t(10));
        assert!((got[0].duration - 20.0).abs() < 1e-6);
        assert_eq!(got[1].timestamp, t(60));
        assert!((got[1].duration - 20.0).abs() < 1e-6);
    }


    #[test]
    fn split_url_extracts_domain_path_protocol() {
        let evs = vec![event(
            t(0),
            1.0,
            json!({"url":"https://www.github.com/Manas-Kenge/Daylog?tab=1"}),
        )];
        let got = split_url_events(evs);
        assert_eq!(got[0].data.get("$domain").unwrap(), &json!("github.com"));
        assert_eq!(got[0].data.get("$protocol").unwrap(), &json!("https"));
        assert_eq!(
            got[0].data.get("$path").unwrap(),
            &json!("/Manas-Kenge/Daylog")
        );
        assert_eq!(got[0].data.get("$params").unwrap(), &json!("tab=1"));
    }

    #[test]
    fn split_url_passes_through_when_no_url() {
        let evs = vec![event(t(0), 1.0, json!({"app":"brave"}))];
        let got = split_url_events(evs);
        assert!(got[0].data.get("$domain").is_none());
    }


    #[test]
    fn categorize_picks_deepest_match() {
        use crate::data::categories::{Category, CategoryConfig, Rule};
        let cfg = CategoryConfig {
            categories: vec![
                Category {
                    name: vec!["Work".into()],
                    rule: Rule::Regex {
                        regex: "code".into(),
                        ignore_case: true,
                    },
                    data: None,
                },
                Category {
                    name: vec!["Work".into(), "Programming".into()],
                    rule: Rule::Regex {
                        regex: "code".into(),
                        ignore_case: true,
                    },
                    data: None,
                },
                Category {
                    name: vec!["Uncategorized".into()],
                    rule: Rule::None,
                    data: None,
                },
            ],
        };
        let rules = compile_rules(&cfg).unwrap();
        let evs = vec![event(t(0), 10.0, json!({"app":"Code","title":"main.rs"}))];
        let got = categorize(evs, &rules);
        assert_eq!(
            got[0].data.get("$category").unwrap(),
            &json!(vec!["Work", "Programming"])
        );
    }

    #[test]
    fn categorize_defaults_to_uncategorized_when_no_match() {
        use crate::data::categories::{Category, CategoryConfig, Rule};
        let cfg = CategoryConfig {
            categories: vec![Category {
                name: vec!["Work".into()],
                rule: Rule::Regex {
                    regex: "nomatch".into(),
                    ignore_case: false,
                },
                data: None,
            }],
        };
        let rules = compile_rules(&cfg).unwrap();
        let evs = vec![event(t(0), 1.0, json!({"app":"brave"}))];
        let got = categorize(evs, &rules);
        assert_eq!(got[0].data.get("$category").unwrap(), &json!(vec!["Uncategorized"]));
    }


    #[test]
    fn flood_merges_same_data_within_pulsetime() {
        let e1 = event(t(0), 1.0, json!({"app":"x"}));
        let e2 = event(t(3), 1.0, json!({"app":"x"}));
        let got = flood(vec![e1, e2], ChronoDuration::seconds(5));
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].timestamp, t(0));
        assert!((got[0].duration - 4.0).abs() < 1e-3);
    }

    #[test]
    fn flood_meet_in_middle_for_different_data() {
        let e1 = event(t(0), 1.0, json!({"app":"a"}));
        let e2 = event(t(3), 1.0, json!({"app":"b"}));
        let got = flood(vec![e1, e2], ChronoDuration::seconds(5));
        assert_eq!(got.len(), 2);
        assert!((got[0].duration - 2.0).abs() < 1e-3);
        assert_eq!(got[1].timestamp, t(2));
        assert!((got[1].duration - 2.0).abs() < 1e-3);
    }

    #[test]
    fn flood_leaves_large_gaps_alone() {
        let e1 = event(t(0), 1.0, json!({"app":"a"}));
        let e2 = event(t(1000), 1.0, json!({"app":"b"}));
        let got = flood(vec![e1.clone(), e2.clone()], ChronoDuration::seconds(5));
        assert_eq!(got.len(), 2);
        assert!((got[0].duration - 1.0).abs() < 1e-3);
        assert_eq!(got[1].timestamp, t(1000));
    }
}
