/**
 * 24h timeline heatmap (96 × 15-min cells). Cells are colored by the
 * dominant category root in their slot.
 *
 * Post-CEO-review (PLAN.md §1.0):
 *   - Yesterday-ghost: yesterday's bucketize96 rendered underneath today
 *     at low opacity. At-a-glance "did I behave the same?" comparison
 *     without leaving the dashboard.
 *   - NOW indicator: vertical line at the current slot.
 *   - AFK stripes: AFK intervals overlay as dim diagonal stripes so the
 *     idle parts of the day are honest rather than hidden.
 */

import { useEffect, useMemo, useState } from "react";
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import {
  useAfkSummary,
  useCategorizedEvents,
} from "@/hooks/useAw";
import { Yesterday } from "@/lib/aw-types";
import { bucketize96 } from "@/lib/timeline";
import { categoryColor } from "@/lib/category-colors";

const CAT_LABELS = [
  "Programming",
  "Documents",
  "Browsing",
  "Comms",
  "Media",
  "Uncategorized",
];
const TOTAL_SLOTS = 96;

export function Timeline() {
  const { data, isLoading } = useCategorizedEvents();
  const { data: yest } = useCategorizedEvents(Yesterday);
  const { data: afk } = useAfkSummary(true);

  const slots = useMemo(() => bucketize96(data ?? []), [data]);
  const yestSlots = useMemo(
    () => bucketize96(yest ?? []),
    [yest],
  );
  const yestHasData = yestSlots.some((s) => s.category != null);

  // AFK slot mask. Slot is "afk" if any AFK interval overlaps its 15-min
  // window (today only — yesterday-ghost intentionally doesn't bother).
  const afkMask = useMemo<boolean[]>(() => {
    const mask = new Array(TOTAL_SLOTS).fill(false);
    for (const it of afk?.intervals ?? []) {
      if (it.status !== "afk") continue;
      const start = new Date(it.timestamp);
      const dayStart = new Date(start);
      dayStart.setHours(0, 0, 0, 0);
      const fromDayStart = (start.getTime() - dayStart.getTime()) / 1000;
      if (fromDayStart < 0 || it.duration <= 0) continue;
      const startSlot = Math.floor(fromDayStart / (15 * 60));
      const endSlot = Math.min(
        TOTAL_SLOTS,
        Math.ceil((fromDayStart + it.duration) / (15 * 60)),
      );
      for (let i = startSlot; i < endSlot && i >= 0; i++) mask[i] = true;
    }
    return mask;
  }, [afk]);

  // NOW indicator: which slot index covers the current wall clock?
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), 60_000);
    return () => clearInterval(id);
  }, []);
  const nowSlot = useMemo(() => {
    const dayStart = new Date(now);
    dayStart.setHours(0, 0, 0, 0);
    const fromDayStart = (now.getTime() - dayStart.getTime()) / 1000;
    return Math.min(TOTAL_SLOTS - 1, Math.floor(fromDayStart / (15 * 60)));
  }, [now]);

  return (
    <Card size="sm" className="flex h-full flex-col">
      <CardHeader className="border-b">
        <CardTitle>Today's timeline</CardTitle>
        <CardDescription>
          96 cells · 15-min resolution · yesterday ghosted below · hover for
          details
        </CardDescription>
        <CardAction>
          <span className="font-mono tabular-nums uppercase tracking-wider text-muted-foreground">
            00:00 → 23:59
          </span>
        </CardAction>
      </CardHeader>

      <CardContent className="flex flex-1 min-h-0 flex-col">
        {isLoading ? (
          <Skeleton className="h-14 w-full rounded-sm" />
        ) : (
          <div className="relative">
            {/* Today row */}
            <div
              className="grid h-14 gap-px overflow-hidden rounded-sm border bg-background"
              style={{ gridTemplateColumns: "repeat(96, 1fr)" }}
            >
              {slots.map((slot) => {
                const hh = String(Math.floor(slot.index / 4)).padStart(2, "0");
                const mm = String((slot.index % 4) * 15).padStart(2, "0");
                const isAfk = afkMask[slot.index];
                const bg = slot.category
                  ? categoryColor([slot.category])
                  : "var(--secondary)";
                const tip =
                  slot.category === null
                    ? `${hh}:${mm} — ${isAfk ? "afk" : "idle"}`
                    : `${hh}:${mm} — ${slot.category}${isAfk ? " (afk overlap)" : ""}`;
                return (
                  <div
                    key={slot.index}
                    className="relative transition-[filter,box-shadow] hover:[filter:brightness(1.6)_saturate(1.2)] hover:shadow-[inset_0_0_0_1px_var(--ring)]"
                    style={{ background: bg }}
                    title={tip}
                  >
                    {isAfk && (
                      <span
                        aria-hidden
                        className="absolute inset-0"
                        style={{
                          background:
                            "repeating-linear-gradient(45deg, transparent 0 2px, var(--background) 2px 4px)",
                          opacity: 0.45,
                        }}
                      />
                    )}
                  </div>
                );
              })}
              {/* NOW indicator overlays the today grid */}
              <div
                aria-hidden
                className="pointer-events-none absolute inset-y-0 w-px bg-foreground/80"
                style={{ left: `${(nowSlot / TOTAL_SLOTS) * 100}%` }}
                title={`now · ${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`}
              />
              <div
                aria-hidden
                className="pointer-events-none absolute -top-1 size-2 rounded-full bg-foreground"
                style={{
                  left: `calc(${(nowSlot / TOTAL_SLOTS) * 100}% - 4px)`,
                }}
              />
            </div>

            {/* Yesterday ghost — only render when yesterday has data */}
            {yestHasData && (
              <div
                className="mt-1 grid h-3 gap-px overflow-hidden rounded-sm border border-dashed bg-background opacity-50"
                style={{ gridTemplateColumns: "repeat(96, 1fr)" }}
                aria-label="Yesterday's timeline (ghosted comparison)"
              >
                {yestSlots.map((slot) => (
                  <div
                    key={slot.index}
                    style={{
                      background: slot.category
                        ? categoryColor([slot.category])
                        : "transparent",
                    }}
                  />
                ))}
              </div>
            )}
          </div>
        )}

        <div className="mt-1.5 flex justify-between font-mono tabular-nums text-muted-foreground">
          <span>00:00</span>
          <span>06:00</span>
          <span>12:00</span>
          <span>18:00</span>
          <span>23:59</span>
        </div>

        <div className="mt-2.5 flex flex-wrap gap-3.5 text-muted-foreground">
          {CAT_LABELS.map((label) => (
            <span key={label} className="inline-flex items-center gap-1.5">
              <span
                className="size-2 rounded-sm"
                style={{ background: categoryColor([label]) }}
              />
              {label}
            </span>
          ))}
          <span className="inline-flex items-center gap-1.5">
            <span
              className="size-2 rounded-sm border border-border"
              style={{
                background:
                  "repeating-linear-gradient(45deg, var(--background) 0 2px, var(--secondary) 2px 4px)",
              }}
            />
            AFK
          </span>
        </div>
      </CardContent>
    </Card>
  );
}
