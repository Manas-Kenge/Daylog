/**
 * Top bar with page title, range switcher, live indicator, refresh.
 */

import { useQueryClient } from "@tanstack/react-query";
import { useRange, type RangePreset } from "@/context/RangeContext";
import { useAfkSummary } from "@/hooks/useAw";
import { fmtClock, fmtDate, fmtDuration } from "@/lib/format";
import { cn } from "@/lib/utils";

const PRESETS: { id: RangePreset; label: string }[] = [
  { id: "today",     label: "Today" },
  { id: "yesterday", label: "Yesterday" },
  { id: "7d",        label: "7d" },
  { id: "30d",       label: "30d" },
  { id: "custom",    label: "Custom" },
];

export function Topbar({ pageTitle }: { pageTitle: string }) {
  const { preset, setPreset } = useRange();
  const queryClient = useQueryClient();
  const now = new Date();
  const { data: afk } = useAfkSummary();

  return (
    <header className="flex items-center justify-between px-[18px] py-[12px] border-b border-border bg-background">
      <div className="flex items-baseline gap-[12px]">
        <h1 className="m-0 text-[18px] font-semibold tracking-tight">{pageTitle}</h1>
        <span className="mono text-muted-foreground text-[12px]">
          {fmtDate(now)} · {fmtClock(now)}
        </span>
      </div>

      <div className="flex items-center gap-[10px]">
        <div role="tablist" className="inline-flex border border-border rounded-[var(--radius)] overflow-hidden bg-card">
          {PRESETS.map((p, i) => (
            <button
              key={p.id}
              type="button"
              onClick={() => setPreset(p.id)}
              className={cn(
                "px-[11px] py-[5px] text-[11.5px] cursor-pointer",
                i < PRESETS.length - 1 && "border-r border-border",
                preset === p.id ? "bg-secondary text-foreground" : "text-muted-foreground hover:text-foreground hover:bg-accent",
              )}
            >
              {p.label}
            </button>
          ))}
        </div>

        <div className="inline-flex items-center gap-[6px] bg-card border border-border rounded-[var(--radius)] px-[10px] py-[5px] text-[11.5px]">
          <span className="w-[6px] h-[6px] rounded-full bg-success shadow-[0_0_6px_var(--success)] animate-[pulse_1.8s_ease-in-out_infinite]" />
          <span className="mono text-foreground">
            {afk ? fmtDuration(afk.active_seconds) : "—"}
          </span>
        </div>

        <button
          type="button"
          onClick={() => queryClient.invalidateQueries()}
          className="bg-card border border-border rounded-[var(--radius)] px-[10px] py-[5px] text-[14px] text-muted-foreground hover:text-foreground hover:bg-accent cursor-pointer"
          aria-label="refresh"
          title="refresh"
        >
          ↻
        </button>
      </div>
    </header>
  );
}
