import { describe, it, expect } from "vitest";
import { trailingStats } from "./baselines";

describe("trailingStats", () => {
  it("returns zeros on empty input", () => {
    expect(trailingStats([], [])).toEqual({
      effectiveDays: 0,
      median: 0,
      mean: 0,
      stdev: 0,
    });
  });

  it("excludes quiet days (active < 30min) from the sample", () => {
    // Three days: two with substantial activity, one quiet.
    const dailyTotals = [3600, 7200, 100];
    const dailyActiveTotals = [4 * 3600, 5 * 3600, 60]; // last one is quiet
    const stats = trailingStats(dailyTotals, dailyActiveTotals);
    expect(stats.effectiveDays).toBe(2);
    expect(stats.median).toBe(5400); // (3600 + 7200) / 2
  });

  it("computes median correctly for odd-length samples", () => {
    const dailyTotals = [100, 200, 300];
    const dailyActiveTotals = [10000, 10000, 10000];
    expect(trailingStats(dailyTotals, dailyActiveTotals).median).toBe(200);
  });

  it("computes stdev as 0 with one effective sample", () => {
    const stats = trailingStats([1000], [10000]);
    expect(stats.stdev).toBe(0);
  });
});
