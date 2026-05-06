import { describe, it, expect } from "vitest";
import { cadence } from "./cadence";
import type { AfkInterval, CategorizedEvent } from "./aw-types";

function ev(hour: number, dur: number): CategorizedEvent {
  const t = new Date();
  t.setHours(hour, 0, 0, 0);
  return { timestamp: t.toISOString(), duration: dur, data: {}, category: ["Work"] };
}

function afkInterval(seconds: number, status: "afk" | "not-afk" = "afk"): AfkInterval {
  return { timestamp: new Date().toISOString(), duration: seconds, status };
}

describe("cadence", () => {
  it("returns nulls on empty input", () => {
    expect(cadence([])).toEqual({ start: null, end: null, idleGaps: 0 });
  });

  it("counts only AFK intervals at or above the floor", () => {
    const intervals = [
      afkInterval(300), // below 600s floor
      afkInterval(601), // counts
      afkInterval(1200, "not-afk"), // wrong status, ignored
      afkInterval(900), // counts
    ];
    const result = cadence([ev(9, 600)], intervals);
    expect(result.idleGaps).toBe(2);
  });

  it("treats a recent last-event as 'still active' (end=null)", () => {
    const t = new Date();
    t.setSeconds(t.getSeconds() - 30);
    const recent: CategorizedEvent = {
      timestamp: t.toISOString(),
      duration: 10,
      data: {},
      category: ["Work"],
    };
    const result = cadence([recent]);
    expect(result.end).toBeNull();
  });

  it("returns end timestamp when last event is far in the past", () => {
    const t = new Date();
    t.setHours(t.getHours() - 5);
    const stale: CategorizedEvent = {
      timestamp: t.toISOString(),
      duration: 60,
      data: {},
      category: ["Work"],
    };
    const result = cadence([stale]);
    expect(result.end).not.toBeNull();
  });
});
