import { describe, it, expect } from "vitest";
import { notableToday, dominantShift } from "./anomaly";

describe("notableToday", () => {
  it("returns empty when no category clears the noise floor", () => {
    // Today: 100s on Work. Trailing: 100s every day. Delta = 0 → noise.
    const trailing = Array.from({ length: 7 }, () => ({
      totals: { Work: 100 },
      activeSec: 4 * 3600,
    }));
    expect(notableToday({ today: { Work: 100 }, trailing })).toEqual([]);
  });

  it("flags a meaningful delta", () => {
    // Today: Browsing 4h, Work 3h. Trailing: 1.5h Browsing typical.
    const trailing = Array.from({ length: 7 }, () => ({
      totals: { Browsing: 1.5 * 3600, Work: 3 * 3600 },
      activeSec: 5 * 3600,
    }));
    const today = { Browsing: 4 * 3600, Work: 3 * 3600 };
    const result = notableToday({ today, trailing });
    expect(result.length).toBeGreaterThan(0);
    expect(result[0].category).toBe("Browsing");
    expect(result[0].direction).toBe("up");
  });

  it("ignores quiet-day baselines (vacation week)", () => {
    // 6 quiet days + 1 normal day → trailing baseline collapses to 1 sample.
    const trailing = [
      { totals: { Work: 5 * 3600 }, activeSec: 5 * 3600 },
      ...Array.from({ length: 6 }, () => ({
        totals: { Work: 60 },
        activeSec: 60, // below quiet-day floor
      })),
    ];
    // Today: 1h Work. Trailing effective median = 5h. Delta = -4h.
    const result = notableToday({ today: { Work: 3600 }, trailing });
    expect(result.length).toBe(1);
    expect(result[0].direction).toBe("down");
    expect(Math.round(result[0].deltaSec / 3600)).toBe(-4);
  });
});

describe("dominantShift", () => {
  it("returns null when nothing notable", () => {
    const trailing = Array.from({ length: 7 }, () => ({
      totals: { Work: 3600 },
      activeSec: 4 * 3600,
    }));
    expect(dominantShift({ today: { Work: 3600 }, trailing })).toBeNull();
  });

  it("returns the largest shift", () => {
    const trailing = Array.from({ length: 7 }, () => ({
      totals: { Browsing: 30 * 60, Work: 5 * 3600 },
      activeSec: 6 * 3600,
    }));
    const today = { Browsing: 3 * 3600, Work: 5 * 3600 };
    const result = dominantShift({ today, trailing });
    expect(result).not.toBeNull();
    expect(result!.category).toBe("Browsing");
  });
});
