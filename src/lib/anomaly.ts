/**
 * "Notable today" anomaly engine.
 *
 * Compares today's per-category totals against a trailing-N-day baseline
 * and surfaces the 1-2 categories with the largest meaningful delta.
 *
 * Meaningful = absolute delta ≥ NOISE_FLOOR (15 minutes) AND the day
 * survived the baseline's quiet-day filter (handled in baselines.ts).
 *
 * Ranking is by Z-score when stdev > 0, falling back to absolute delta
 * when there's only one or two baseline samples (stdev = 0). This avoids
 * the divide-by-zero degenerate case while still ranking sensibly with
 * little history.
 */

import { trailingStats, PATTERN_SHIFT_NOISE_FLOOR_SEC } from "./baselines";

export interface CategoryTotals {
  /** Mapped: category root → total seconds. */
  [root: string]: number;
}

export interface Anomaly {
  category: string;
  todaySec: number;
  medianSec: number;
  deltaSec: number;
  /** Positive = more than typical, negative = less. */
  direction: "up" | "down";
  /** Z-score against trailing baseline. 0 when stdev is 0. */
  zScore: number;
}

export interface NotableTodayInput {
  /** Today's category-root → seconds map. */
  today: CategoryTotals;
  /** Per-day arrays: index i = i days ago (1..N). Each day is its own
   *  category map, plus the day's overall active total for the
   *  quiet-day filter. */
  trailing: ReadonlyArray<{ totals: CategoryTotals; activeSec: number }>;
}

/**
 * Returns up to `limit` anomalies, ranked by Z-score (or |delta| when
 * stdev is zero). Filters out below-noise-floor deltas. Empty when no
 * category meets the bar.
 */
export function notableToday(input: NotableTodayInput, limit = 2): Anomaly[] {
  const categories = new Set<string>(Object.keys(input.today));
  for (const day of input.trailing) {
    for (const k of Object.keys(day.totals)) categories.add(k);
  }

  const dailyActiveTotals = input.trailing.map((d) => d.activeSec);

  const ranked: Anomaly[] = [];
  for (const cat of categories) {
    const todaySec = input.today[cat] ?? 0;
    const dailyTotals = input.trailing.map((d) => d.totals[cat] ?? 0);
    const stats = trailingStats(dailyTotals, dailyActiveTotals);
    if (stats.effectiveDays === 0) continue;
    const deltaSec = todaySec - stats.median;
    if (Math.abs(deltaSec) < PATTERN_SHIFT_NOISE_FLOOR_SEC) continue;
    const zScore = stats.stdev > 0 ? (todaySec - stats.mean) / stats.stdev : 0;
    ranked.push({
      category: cat,
      todaySec,
      medianSec: stats.median,
      deltaSec,
      direction: deltaSec >= 0 ? "up" : "down",
      zScore,
    });
  }

  ranked.sort((a, b) => {
    const aRank = a.zScore !== 0 ? Math.abs(a.zScore) : Math.abs(a.deltaSec) / 3600;
    const bRank = b.zScore !== 0 ? Math.abs(b.zScore) : Math.abs(b.deltaSec) / 3600;
    return bRank - aRank;
  });

  return ranked.slice(0, limit);
}

/**
 * Largest-absolute-delta anomaly, used by the "Pattern shift" KPI sub-line.
 * Returns null when nothing meaningful has shifted.
 */
export function dominantShift(input: NotableTodayInput): Anomaly | null {
  const top = notableToday(input, 1);
  return top.length > 0 ? top[0] : null;
}
