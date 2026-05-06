/**
 * "Best Window" KPI computation.
 *
 * For a given day's categorized events, find the contiguous hour-range
 * with the highest concentration of *focused* time (qualifying focus runs
 * ≥ floorSec on a single category root). Returns {startHour, endHour,
 * seconds}, or null when there's no focused time.
 *
 * Algorithm: compute focusByHour, then scan all WINDOW_HOURS-length
 * sliding windows, returning the one with the highest sum. Ties broken
 * by earliest start.
 *
 * The window length is fixed at 3 hours — long enough to feel like a
 * meaningful "focus block," short enough that "best window" still
 * resolves to a specific time-of-day. Smaller windows degenerate to
 * "best hour"; larger windows blur into the whole day.
 */

import type { CategorizedEvent } from "./aw-types";
import { focusByHour } from "./kpi";

const WINDOW_HOURS = 3;

export interface BestWindow {
  startHour: number;
  endHour: number;
  seconds: number;
}

export function bestWindow(
  events: readonly CategorizedEvent[],
  floorSec = 120,
): BestWindow | null {
  const perHour = focusByHour(events, floorSec);
  let bestStart = 0;
  let bestSum = 0;
  for (let start = 0; start <= 24 - WINDOW_HOURS; start++) {
    let sum = 0;
    for (let h = start; h < start + WINDOW_HOURS; h++) sum += perHour[h];
    if (sum > bestSum) {
      bestSum = sum;
      bestStart = start;
    }
  }
  if (bestSum === 0) return null;
  return {
    startHour: bestStart,
    endHour: bestStart + WINDOW_HOURS,
    seconds: bestSum,
  };
}
