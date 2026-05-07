/**
 * Pure helpers for KPI-strip computations. Exported separately from
 * the widget so they can be tested without React.
 */

import type { CategorizedEvent } from "./aw-types";
import { categoryRoot } from "./category-colors";

/**
 * Longest contiguous focus session today. A "session" = consecutive events
 * sharing the same category root. Below the floor we don't count it as
 * focus. Returns total seconds of the longest qualifying run, plus the
 * root it was on (for the card sub-label).
 */
export function longestFocus(
  events: readonly CategorizedEvent[],
  floorSec = 120,
): { seconds: number; root: string | null } {
  if (events.length === 0) return { seconds: 0, root: null };

  // Grouping needs chronological order.
  const sorted = [...events].sort((a, b) =>
    a.timestamp < b.timestamp ? -1 : 1,
  );

  let bestSec = 0;
  let bestRoot: string | null = null;
  let runSec = 0;
  let runRoot: string | null = null;

  for (const ev of sorted) {
    const root = categoryRoot(ev.category);
    if (root !== runRoot) {
      if (runSec >= floorSec && runSec > bestSec) {
        bestSec = runSec;
        bestRoot = runRoot;
      }
      runRoot = root;
      runSec = ev.duration;
    } else {
      runSec += ev.duration;
    }
  }
  if (runSec >= floorSec && runSec > bestSec) {
    bestSec = runSec;
    bestRoot = runRoot;
  }
  return { seconds: bestSec, root: bestRoot };
}

/**
 * Per-hour focused-time spark (24 entries). Counts only event duration that
 * falls inside a run ≥ floorSec on a single category root. The hourly Active
 * chart already exists; this one is filtered to qualifying focus runs only,
 * so the bar peaks reveal when the user actually had deep stretches.
 */
export function focusByHour(
  events: readonly CategorizedEvent[],
  floorSec = 120,
): number[] {
  const out = new Array(24).fill(0);
  if (events.length === 0) return out;

  const sorted = [...events].sort((a, b) =>
    a.timestamp < b.timestamp ? -1 : 1,
  );

  let runStart = 0;
  let runRoot: string | null = null;
  let runSec = 0;

  const flush = (endIdx: number) => {
    if (runSec >= floorSec && runRoot !== null) {
      for (let j = runStart; j < endIdx; j++) {
        const h = new Date(sorted[j].timestamp).getHours();
        if (h >= 0 && h < 24) out[h] += sorted[j].duration;
      }
    }
  };

  for (let i = 0; i < sorted.length; i++) {
    const root = categoryRoot(sorted[i].category);
    if (root !== runRoot) {
      flush(i);
      runStart = i;
      runRoot = root;
      runSec = sorted[i].duration;
    } else {
      runSec += sorted[i].duration;
    }
  }
  flush(sorted.length);
  return out;
}

