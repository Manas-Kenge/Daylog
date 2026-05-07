/**
 * Topbar. Thin, view-aware.
 *
 * Left:  back-button (when off-Overview) · scoped active total · delta (today only)
 * Right: view toggle (Today / Week / Month)
 *
 * The toggle is the only navigation surface — Overview, Week, and Month
 * are the only pages.
 */

import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { usePage, type PageId } from "@/context/PageContext";
import { useScopedActive } from "@/hooks/useAw";
import { fmtDuration } from "@/lib/format";

type ViewKey = "today" | "week" | "month";

const VIEW_TABS: Array<{ value: ViewKey; label: string }> = [
  { value: "today", label: "Today" },
  { value: "week", label: "Week" },
  { value: "month", label: "Month" },
];

function pageToView(page: PageId): ViewKey {
  return page === "overview" ? "today" : page;
}

const SCOPE_LABEL: Record<"today" | "7-day" | "30-day", string> = {
  today: "today",
  "7-day": "in 7 days",
  "30-day": "in 30 days",
};

export function Topbar() {
  const { page, push } = usePage();

  const currentView = pageToView(page);
  const scoped = useScopedActive(currentView);
  const { activeSec, delta, isLoading: scopedLoading } = scoped;
  const haveDelta = delta !== undefined && delta !== 0;

  const onView = (next: ViewKey) => {
    if (next === "today") push("overview");
    else if (next === "week") push("week");
    else push("month");
  };

  return (
    <header className="flex min-w-0 items-center justify-between gap-3.5 border-b bg-background px-4 py-2.5">
      <div className="flex min-w-0 shrink-0 items-center gap-3">
        {scopedLoading ? (
          <Skeleton className="h-4 w-20" />
        ) : (
          <span className="font-mono tabular-nums text-foreground">
            {fmtDuration(activeSec)} active{" "}
            <span className="text-muted-foreground">
              {SCOPE_LABEL[scoped.label]}
            </span>
          </span>
        )}
        {haveDelta && (
          <Badge
            variant={delta! > 0 ? "outline" : "destructive"}
            className="h-6 font-mono tabular-nums"
          >
            {delta! > 0 ? "↑" : "↓"} {fmtDuration(Math.abs(delta!))}
          </Badge>
        )}
      </div>

      <ToggleGroup
        type="single"
        size="sm"
        value={currentView}
        onValueChange={(v) => {
          if (v) onView(v as ViewKey);
        }}
        aria-label="View"
        className="shrink-0"
      >
        {VIEW_TABS.map((t) => (
          <ToggleGroupItem key={t.value} value={t.value}>
            {t.label}
          </ToggleGroupItem>
        ))}
      </ToggleGroup>
    </header>
  );
}
