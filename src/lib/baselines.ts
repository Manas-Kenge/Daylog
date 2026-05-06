/**
 * Trailing-N-day baseline statistics.
 *
 * The "Pattern shift" KPI sub-line ("vs typical Tue") and "Notable today"
 * widget both compare today against a baseline computed over the previous
 * N days. These pure helpers do that computation; the data fetch happens
 * in `useTrailingDays` (hooks/useAw.ts).
 *
 * Quiet-day filter: a day with < QUIET_DAY_FLOOR seconds of total activity
 * is excluded from the baseline. Without this, vacation days, weekends,
 * or paused-tracking days would skew the median toward zero and make the
 * "vs typical" delta meaningless.
 */

const QUIET_DAY_FLOOR = 30 * 60; // 30 minutes

export interface BaselineStats {
  /** Days that survived the quiet-day filter — what the median is over. */
  effectiveDays: number;
  median: number;
  mean: number;
  /** Sample standard deviation. 0 when fewer than 2 effective days. */
  stdev: number;
}

/**
 * Compute baseline stats from a stream of per-day metric values.
 *
 * `dailyTotals` is a parallel array: index i is the relevant total for day i
 * back. Caller supplies whichever metric they want a baseline for (active
 * seconds, work seconds, longest-stretch seconds, etc.).
 *
 * `dailyActiveTotals` is each day's overall active total, used solely for
 * the quiet-day filter — a day that was "quiet overall" gets excluded
 * regardless of which metric we're computing the baseline for.
 */
export function trailingStats(
  dailyTotals: readonly number[],
  dailyActiveTotals: readonly number[],
): BaselineStats {
  const samples: number[] = [];
  const len = Math.min(dailyTotals.length, dailyActiveTotals.length);
  for (let i = 0; i < len; i++) {
    if (dailyActiveTotals[i] >= QUIET_DAY_FLOOR) samples.push(dailyTotals[i]);
  }
  const effectiveDays = samples.length;
  if (effectiveDays === 0) {
    return { effectiveDays: 0, median: 0, mean: 0, stdev: 0 };
  }
  const sorted = [...samples].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  const median =
    sorted.length % 2 === 0 ? (sorted[mid - 1] + sorted[mid]) / 2 : sorted[mid];
  const mean = samples.reduce((a, b) => a + b, 0) / samples.length;
  let variance = 0;
  if (samples.length >= 2) {
    let sumSq = 0;
    for (const v of samples) sumSq += (v - mean) ** 2;
    variance = sumSq / (samples.length - 1);
  }
  const stdev = Math.sqrt(variance);
  return { effectiveDays, median, mean, stdev };
}

/**
 * Threshold below which "vs typical" deltas should be suppressed in the UI.
 * Surfacing "+47 seconds vs typical" is noise.
 */
export const PATTERN_SHIFT_NOISE_FLOOR_SEC = 15 * 60;
