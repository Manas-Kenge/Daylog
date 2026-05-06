/**
 * Topbar (palette-primary). Per PLAN §5 + §1.0 addendum: thin, view-aware.
 *
 * Left:    Pulse mark · separator · back-button (when off-Overview) · scoped
 *          active total · delta (today only)
 * Right:   View toggle (Today / Week / Month) · ⌘K hint
 *
 * The view toggle is the discoverable surface for navigating between
 * Overview, Week page, and Month page. It does NOT change RangeContext
 * for arbitrary ranges — palette commands handle Yesterday and custom
 * ranges. The toggle is visible on all three views so the user can hop
 * between them without opening the palette.
 */

import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowLeft02Icon } from "@hugeicons/core-free-icons";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { useRange } from "@/context/RangeContext";
import { usePage, type PageId, PAGE_TITLES } from "@/context/PageContext";
import { useScopedActive } from "@/hooks/useAw";
import { fmtDuration } from "@/lib/format";

type ViewKey = "today" | "week" | "month";

const VIEW_TABS: Array<{ value: ViewKey; label: string }> = [
  { value: "today", label: "Today" },
  { value: "week", label: "Week" },
  { value: "month", label: "Month" },
];

/** Which view is the user currently looking at? Pages that aren't part of
 *  the toggle group (apps, categories, hourly, etc.) yield null so the
 *  toggle has no selected option. */
function pageToView(page: PageId): ViewKey | null {
  if (page === "overview") return "today";
  if (page === "week") return "week";
  if (page === "month") return "month";
  return null;
}

function viewToScope(view: ViewKey): "today" | "week" | "month" {
  return view;
}

const SCOPE_LABEL: Record<"today" | "7-day" | "30-day", string> = {
  today: "today",
  "7-day": "in 7 days",
  "30-day": "in 30 days",
};

export function Topbar() {
  const { setPreset } = useRange();
  const { page, push, back } = usePage();

  const currentView = pageToView(page);
  // For pages outside the toggle group (apps, categories, etc.), keep the
  // active total scoped to today so the topbar reads as a "context badge."
  const scopeForStat = currentView ? viewToScope(currentView) : "today";
  const scoped = useScopedActive(scopeForStat);
  const { activeSec, delta, isLoading: scopedLoading } = scoped;
  const haveDelta = delta !== undefined && delta !== 0;

  const onView = (next: ViewKey) => {
    if (next === "today") {
      // Reset RangeContext to today so Overview's range-aware widgets
      // don't leak from a prior palette pick like "yesterday."
      setPreset("today");
      push("overview");
    } else if (next === "week") {
      push("week");
    } else {
      push("month");
    }
  };

  return (
    <header className="flex min-w-0 items-center justify-between gap-3.5 border-b bg-background px-4 py-2.5">
      {/* Left — brand + back-button (or active total) + delta */}
      <div className="flex min-w-0 shrink-0 items-center gap-3">
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

      {/* Right — view toggle (Today/Week/Month) + ⌘K */}
      <div className="flex shrink-0 items-center gap-2">
        <ToggleGroup
          type="single"
          size="sm"
          value={currentView ?? ""}
          onValueChange={(v) => {
            if (v) onView(v as ViewKey);
          }}
          aria-label="View"
        >
          {VIEW_TABS.map((t) => (
            <ToggleGroupItem key={t.value} value={t.value}>
              {t.label}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
        <Badge variant="outline" className="h-6 font-mono tabular-nums" title="Open command palette">
          ⌘K
        </Badge>
      </div>
    </header>
  );
}
