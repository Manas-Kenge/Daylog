/**
 * "Cadence" KPI — the day's narrative shape.
 *
 * Replaces the v0.1-original `Started` KPI (PLAN.md §1.0): one timestamp
 * doesn't tell the day's story, but start + end + idle gaps does.
 *
 * Inputs:
 *   - categorized events (gives us first/last activity timestamps)
 *   - AFK intervals (gives us idle gaps between active stretches)
 *
 * Outputs: start / end (or null if active) / count of idle gaps ≥ floorSec.
 * Default floor 10 minutes — shorter than a coffee break, longer than
 * an inter-window pause.
 */

import type { AfkInterval, CategorizedEvent } from "./aw-types";

export interface Cadence {
  /** First activity of the day. Null if no events. */
  start: Date | null;
  /** Last activity of the day. Null if currently active or no events. */
  end: Date | null;
  /** Number of idle gaps ≥ floorSec between active stretches. */
  idleGaps: number;
}

export function cadence(
  events: readonly CategorizedEvent[],
  intervals: readonly AfkInterval[] = [],
  floorSec = 600,
  /** Tolerance (sec) for "is the user still active right now?" — within
   *  this many seconds of the current wall clock, end stays null. */
  liveTailSec = 5 * 60,
): Cadence {
  if (events.length === 0) {
    return { start: null, end: null, idleGaps: 0 };
  }

  let firstTs = events[0].timestamp;
  let lastTs = events[0].timestamp;
  let lastDur = events[0].duration;
  for (const ev of events) {
    if (ev.timestamp < firstTs) firstTs = ev.timestamp;
    if (ev.timestamp > lastTs) {
      lastTs = ev.timestamp;
      lastDur = ev.duration;
    }
  }
  const start = new Date(firstTs);
  const lastEvEnd = new Date(new Date(lastTs).getTime() + lastDur * 1000);

  // Active "right now" if the last event ended within liveTailSec.
  const now = Date.now();
  const isLive = now - lastEvEnd.getTime() < liveTailSec * 1000;

  const end = isLive ? null : lastEvEnd;

  let idleGaps = 0;
  for (const it of intervals) {
    if (it.status === "afk" && it.duration >= floorSec) idleGaps++;
  }

  return { start, end, idleGaps };
}
