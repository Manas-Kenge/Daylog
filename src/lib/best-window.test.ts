import { describe, it, expect } from "vitest";
import { bestWindow } from "./best-window";
import type { CategorizedEvent } from "./aw-types";

function ev(hour: number, dur: number, category: string[] = ["Work"]): CategorizedEvent {
  const ts = new Date();
  ts.setHours(hour, 0, 0, 0);
  return { timestamp: ts.toISOString(), duration: dur, data: {}, category };
}

describe("bestWindow", () => {
  it("returns null when there is no focus time", () => {
    expect(bestWindow([])).toBeNull();
  });

  it("returns null when every run is below the focus floor", () => {
    // Single 60s event on Work — below the 120s floor → not focused.
    expect(bestWindow([ev(14, 60)])).toBeNull();
  });

  it("finds the densest 3-hour window", () => {
    // Heavy run 14-17, light run 09-10. Best window should be 14-17.
    const events = [
      ev(9, 600), // 10 min, qualifies as focus run on its own (≥120s)
      ev(14, 1800), // 30m
      ev(15, 1800),
      ev(16, 1800),
    ];
    const result = bestWindow(events);
    expect(result).not.toBeNull();
    expect(result!.startHour).toBe(14);
    expect(result!.endHour).toBe(17);
  });

  it("breaks ties by earliest start", () => {
    const events = [
      ev(9, 1200),
      ev(10, 1200),
      ev(11, 1200),
      ev(20, 1200),
      ev(21, 1200),
      ev(22, 1200),
    ];
    const result = bestWindow(events);
    expect(result!.startHour).toBe(9);
  });
});
