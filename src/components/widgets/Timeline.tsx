/**
 * 24h timeline heatmap (96 × 15-min cells). Cells are colored by the
 * dominant category root in their slot.
 */

import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { useCategorizedEvents } from "@/hooks/useAw";
import { bucketize96 } from "@/lib/timeline";
import { categoryColor } from "@/lib/category-colors";
import { useMemo } from "react";

const CAT_LABELS = ["Programming", "Documents", "Browsing", "Comms", "Media", "Uncategorized"];

export function Timeline() {
  const { data, isLoading } = useCategorizedEvents();
  const slots = useMemo(() => bucketize96(data ?? []), [data]);

  return (
    <Card size="sm">
      <CardHeader className="border-b">
        <CardTitle>Today's timeline</CardTitle>
        <CardDescription>
          96 cells · 15-min resolution · hover for details
        </CardDescription>
        <CardAction>
          <span className="font-mono tabular-nums uppercase tracking-wider text-muted-foreground">
            00:00 → 23:59
          </span>
        </CardAction>
      </CardHeader>

      <CardContent>
        {isLoading ? (
          <Skeleton className="h-14 w-full rounded-sm" />
        ) : (
        <div
          className="grid h-14 gap-px overflow-hidden rounded-sm border bg-background"
          style={{ gridTemplateColumns: "repeat(96, 1fr)" }}
        >
          {slots.map((slot) => {
            const hh = String(Math.floor(slot.index / 4)).padStart(2, "0");
            const mm = String((slot.index % 4) * 15).padStart(2, "0");
            const bg = slot.category
              ? categoryColor([slot.category])
              : "var(--secondary)";
            const tip =
              slot.category === null
                ? `${hh}:${mm} — idle / AFK`
                : `${hh}:${mm} — ${slot.category}`;
            return (
              <div
                key={slot.index}
                className="cursor-crosshair transition-[filter,box-shadow] hover:[filter:brightness(1.6)_saturate(1.2)] hover:shadow-[inset_0_0_0_1px_var(--ring)]"
                style={{ background: bg }}
                title={tip}
              />
            );
          })}
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
        </div>
      </CardContent>
    </Card>
  );
}
