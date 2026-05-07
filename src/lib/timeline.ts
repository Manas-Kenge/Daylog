// Dominant category (longest contributor) wins per 15-min slot.

import type { CategorizedEvent } from "./aw-types";
import { categoryRoot } from "./category-colors";

export interface TimelineSlot {
  index: number;
  startSec: number;
  category: string | null;
  durationSec: number;
}

const SLOT_SEC = 15 * 60;
const TOTAL_SLOTS = 96;

export function bucketize96(events: CategorizedEvent[]): TimelineSlot[] {
  const slots: TimelineSlot[] = Array.from({ length: TOTAL_SLOTS }, (_, i) => ({
    index: i,
    startSec: i * SLOT_SEC,
    category: null,
    durationSec: 0,
  }));
  const tallies = new Map<number, Record<string, number>>();

  for (const ev of events) {
    const start = new Date(ev.timestamp);
    const dayStart = new Date(start);
    dayStart.setHours(0, 0, 0, 0);
    const fromDayStart = (start.getTime() - dayStart.getTime()) / 1000;
    if (fromDayStart < 0 || ev.duration <= 0) continue;
    const cat = categoryRoot(ev.category);

    let remaining = ev.duration;
    let cursor = fromDayStart;
    for (let safety = 0; safety < 200 && remaining > 0; safety++) {
      const slotIdx = Math.floor(cursor / SLOT_SEC);
      if (slotIdx >= TOTAL_SLOTS) break;
      const nextBoundary = (slotIdx + 1) * SLOT_SEC;
      const chunk = Math.min(remaining, nextBoundary - cursor);
      const tally = tallies.get(slotIdx) ?? {};
      tally[cat] = (tally[cat] ?? 0) + chunk;
      tallies.set(slotIdx, tally);
      remaining -= chunk;
      cursor = nextBoundary;
    }
  }

  for (const [idx, tally] of tallies) {
    let best = "";
    let bestVal = 0;
    let total = 0;
    for (const [k, v] of Object.entries(tally)) {
      total += v;
      if (v > bestVal) {
        bestVal = v;
        best = k;
      }
    }
    slots[idx].durationSec = total;
    slots[idx].category = best || null;
  }
  return slots;
}
