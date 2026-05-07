/**
 * Window length is fixed at 3 hours — long enough to feel like a
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
