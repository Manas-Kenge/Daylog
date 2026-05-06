/**
 * Topbar (palette-primary). Per PLAN §5: thin, range-aware, no nav buttons.
 *
 * Left:    Pulse mark · separator · range label / back-button · today total · delta
 * Right:   range ToggleGroup (only on Overview) · ⌘K hint
 */

import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowLeft02Icon } from "@hugeicons/core-free-icons";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { useRange, type RangePreset } from "@/context/RangeContext";
import { usePage, PAGE_TITLES } from "@/context/PageContext";
import { useAfkTodayVsYesterday } from "@/hooks/useAw";
import { fmtDuration } from "@/lib/format";

const RANGE_TABS: Array<{ value: Exclude<RangePreset, "custom">; label: string }> = [
  { value: "today", label: "Today" },
  { value: "yesterday", label: "Yesterday" },
  { value: "7d", label: "Week" },
  { value: "30d", label: "Month" },
];

export function Topbar() {
  const { preset, setPreset } = useRange();
  const { page, back } = usePage();

  const { today: afkToday, yesterday: afkYest } = useAfkTodayVsYesterday();

  const activeSec = afkToday.data?.active_seconds ?? 0;
  const yestSec = afkYest.data?.active_seconds ?? 0;
  const delta = activeSec - yestSec;
  const haveYest = afkYest.data !== undefined;

  return (
    <header className="flex min-w-0 items-center justify-between gap-3.5 border-b bg-background px-4 py-2.5">
      {/* Left — brand + back-button (or active total) + delta */}
      <div className="flex min-w-0 shrink-0 items-center gap-3">
        <div className="flex items-center gap-2">
          <div className="relative size-5 rounded-md border bg-secondary">
            <span className="absolute inset-0 m-auto size-1.5 rounded-full bg-foreground" />
          </div>
          <span className="font-semibold tracking-tight">Pulse</span>
        </div>
        <Separator orientation="vertical" className="h-4" />
        {page !== "overview" && (
          <button
            type="button"
            onClick={back}
            className="inline-flex cursor-pointer items-center gap-1.5 text-muted-foreground hover:text-foreground"
            title="back to Overview"
          >
            <HugeiconsIcon icon={ArrowLeft02Icon} size={14} />
            <span>{PAGE_TITLES[page]}</span>
          </button>
        )}
        {afkToday.isLoading ? (
          <Skeleton className="h-4 w-20" />
        ) : (
          <span className="font-mono tabular-nums text-foreground">
            {fmtDuration(activeSec)} active
          </span>
        )}
        {haveYest && delta !== 0 && (
          <Badge
            variant={delta > 0 ? "outline" : "destructive"}
            className="h-6 font-mono tabular-nums"
          >
            {delta > 0 ? "↑" : "↓"} {fmtDuration(Math.abs(delta))}
          </Badge>
        )}
      </div>

      {/* Right — range switcher + ⌘K */}
      <div className="flex shrink-0 items-center gap-2">
        {page === "overview" && (
          <ToggleGroup
            type="single"
            size="sm"
            value={preset}
            onValueChange={(v) => {
              if (v) setPreset(v as RangePreset);
            }}
            aria-label="Time range"
          >
            {RANGE_TABS.map((t) => (
              <ToggleGroupItem key={t.value} value={t.value}>
                {t.label}
              </ToggleGroupItem>
            ))}
          </ToggleGroup>
        )}
        <Badge variant="outline" className="h-6 font-mono tabular-nums" title="Open command palette">
          ⌘K
        </Badge>
      </div>
    </header>
  );
}
