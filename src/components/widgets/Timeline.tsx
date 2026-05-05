/**
 * 24h timeline heatmap (96 × 15-min cells). Cells are colored by the
 * dominant category root in their slot. Wrapped in the databuddy tray
 * pattern (bg-secondary p-1.5 rounded-xl outer, rounded-lg card inner).
 */

import { useCategorizedEvents } from "@/hooks/useAw";
import { bucketize96 } from "@/lib/timeline";
import { categoryColor } from "@/lib/category-colors";
import { useMemo } from "react";

const CAT_LABELS = ["Programming", "Documents", "Browsing", "Comms", "Media", "Uncategorized"];

export function Timeline() {
  const { data } = useCategorizedEvents();
  const slots = useMemo(() => bucketize96(data ?? []), [data]);

  return (
    <div className="rounded-[var(--radius-xl)] bg-secondary p-[6px]">
      <div className="rounded-[var(--radius-lg)] border border-sidebar-border bg-card overflow-hidden">
        <header className="flex items-center justify-between px-[14px] py-[10px] border-b border-sidebar-border bg-sidebar/50">
          <div>
            <div className="font-semibold text-[14px] text-foreground">Today's timeline</div>
            <div className="text-[11.5px] text-muted-foreground">
              96 cells · 15-min resolution · hover for details
            </div>
          </div>
          <span className="text-[10.5px] tracking-[0.13em] uppercase text-muted-foreground font-medium mono">
            00:00 → 23:59
          </span>
        </header>

        <div className="px-[14px] pt-[12px] pb-[12px]">
          <div
            className="grid gap-px h-[56px] border border-border rounded-[var(--radius-sm)] overflow-hidden bg-background"
            style={{ gridTemplateColumns: "repeat(96, 1fr)" }}
          >
            {slots.map((slot) => {
              const hh = String(Math.floor(slot.index / 4)).padStart(2, "0");
              const mm = String((slot.index % 4) * 15).padStart(2, "0");
              const bg = slot.category ? categoryColor([slot.category]) : "var(--secondary)";
              const tip =
                slot.category === null
                  ? `${hh}:${mm} — idle / AFK`
                  : `${hh}:${mm} — ${slot.category}`;
              return (
                <div
                  key={slot.index}
                  className="cursor-crosshair transition-[filter,box-shadow] hover:[filter:brightness(1.6)_saturate(1.2)] hover:shadow-[inset_0_0_0_1px_var(--brand-coral)]"
                  style={{ background: bg }}
                  title={tip}
                />
              );
            })}
          </div>

          <div className="flex justify-between mt-[6px] text-muted-foreground text-[10.5px] mono">
            <span>00:00</span>
            <span>06:00</span>
            <span>12:00</span>
            <span>18:00</span>
            <span>23:59</span>
          </div>

          <div className="flex flex-wrap gap-[14px] mt-[10px] text-[11px] text-muted-foreground">
            {CAT_LABELS.map((label) => (
              <span key={label} className="inline-flex items-center gap-[5px]">
                <span
                  className="w-[9px] h-[9px] rounded-[2px]"
                  style={{ background: categoryColor([label]) }}
                />
                {label}
              </span>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
