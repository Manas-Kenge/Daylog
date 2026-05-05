/**
 * Current focus session + AFK ratio. Pulls the latest event run from
 * categorized_events and the AFK summary.
 */

import { WidgetCard } from "./Card";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { useAfkSummary, useCategorizedEvents } from "@/hooks/useAw";
import { currentSession } from "@/lib/timeline";
import { categoryLabel } from "@/lib/category-colors";
import { fmtClock, fmtDuration, fmtPercent } from "@/lib/format";

export function CurrentFocus() {
  const { data: events, isLoading: eventsLoading } = useCategorizedEvents();
  const { data: afk, isLoading: afkLoading } = useAfkSummary();
  const loading = eventsLoading || afkLoading;

  const session = events ? currentSession(events) : null;

  // Window-switch count for the current session = events sharing the run.
  let switchesInSession = 0;
  if (events && session) {
    for (const ev of events) {
      const t = new Date(ev.timestamp);
      if (t >= session.start) switchesInSession++;
    }
  }

  // Focus ring: progress against a 60-min default goal (visual only).
  const sessionMin = session ? Math.floor(session.durationSec / 60) : 0;
  const goalMin = 60;
  const focusFrac = Math.min(1, sessionMin / goalMin);
  const FOCUS_R = 26;
  const FOCUS_C = 2 * Math.PI * FOCUS_R;

  // AFK ring
  const ratio = afk?.active_ratio ?? 0;
  const AFK_R = 26;
  const AFK_C = 2 * Math.PI * AFK_R;

  return (
    <WidgetCard
      title="Current focus & AFK"
      description="Live session and active ratio"
    >
      {loading ? (
        <div className="flex flex-col gap-3">
          <div className="flex items-center gap-3">
            <Skeleton className="size-14 rounded-full" />
            <div className="flex min-w-0 flex-1 flex-col gap-1.5">
              <Skeleton className="h-4 w-2/3" />
              <Skeleton className="h-3 w-3/4" />
            </div>
          </div>
          <div className="flex flex-col gap-1.5">
            <Skeleton className="h-3 w-full" />
            <Skeleton className="h-3 w-full" />
            <Skeleton className="h-3 w-full" />
          </div>
          <Separator />
          <div className="flex items-center gap-3.5">
            <Skeleton className="size-16 rounded-full" />
            <div className="flex min-w-0 flex-1 flex-col gap-1.5">
              <Skeleton className="h-3 w-full" />
              <Skeleton className="h-3 w-full" />
              <Skeleton className="h-3 w-full" />
              <Skeleton className="mt-1 h-1 w-full" />
            </div>
          </div>
        </div>
      ) : (
      <>
      <div className="flex items-center gap-3">
        <svg viewBox="0 0 64 64" className="size-14 shrink-0">
          <circle cx="32" cy="32" r={FOCUS_R} fill="none" stroke="var(--secondary)" strokeWidth="3" />
          <circle
            cx="32"
            cy="32"
            r={FOCUS_R}
            fill="none"
            stroke="var(--foreground)"
            strokeWidth="3"
            strokeDasharray={FOCUS_C}
            strokeDashoffset={FOCUS_C * (1 - focusFrac)}
            strokeLinecap="round"
            transform="rotate(-90 32 32)"
          />
          <text
            x="32"
            y="35"
            textAnchor="middle"
            fontFamily="ui-monospace, monospace"
            fontSize="11"
            fontWeight="600"
            fill="currentColor"
          >
            {sessionMin}m
          </text>
        </svg>
        <div className="flex min-w-0 flex-col gap-0.5">
          <div className="font-mono tabular-nums text-base font-semibold tracking-tight">
            {session?.app || "—"}
          </div>
          <div className="max-w-[220px] truncate text-muted-foreground">
            {session?.title || "no recent activity"}
          </div>
        </div>
      </div>

      <div className="mt-3 flex flex-col">
        <Stat k="started" v={session ? <span className="font-mono tabular-nums">{fmtClock(session.start)}</span> : "—"} first />
        <Stat k="window switches" v={<span className="font-mono tabular-nums">{switchesInSession || "—"}</span>} />
        <Stat
          k="category"
          v={session ? <span>{categoryLabel(session.category)}</span> : "—"}
        />
      </div>

      <Separator className="my-3" />

      <div className="flex items-center gap-3.5">
        <svg viewBox="0 0 64 64" className="size-16 shrink-0">
          <circle cx="32" cy="32" r={AFK_R} fill="none" stroke="var(--secondary)" strokeWidth="5" />
          <circle
            cx="32"
            cy="32"
            r={AFK_R}
            fill="none"
            stroke="var(--foreground)"
            strokeWidth="5"
            strokeDasharray={AFK_C}
            strokeDashoffset={AFK_C * (1 - ratio)}
            strokeLinecap="butt"
            transform="rotate(-90 32 32)"
          />
          <text
            x="32"
            y="35"
            textAnchor="middle"
            fontFamily="ui-monospace, monospace"
            fontSize="11"
            fontWeight="600"
            fill="currentColor"
          >
            {fmtPercent(ratio)}
          </text>
        </svg>
        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <div className="flex justify-between">
            <span className="text-muted-foreground">active</span>
            <span className="font-mono tabular-nums font-medium">{fmtDuration(afk?.active_seconds ?? 0)}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-muted-foreground">afk</span>
            <span className="font-mono tabular-nums font-medium">{fmtDuration(afk?.afk_seconds ?? 0)}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-muted-foreground">tracked</span>
            <span className="font-mono tabular-nums font-medium">
              {fmtDuration((afk?.active_seconds ?? 0) + (afk?.afk_seconds ?? 0))}
            </span>
          </div>
          <div className="mt-1 flex h-1 overflow-hidden rounded-sm bg-secondary">
            <div className="h-full bg-foreground" style={{ width: `${(ratio * 100).toFixed(1)}%` }} />
            <div className="h-full bg-accent" style={{ width: `${((1 - ratio) * 100).toFixed(1)}%` }} />
          </div>
        </div>
      </div>
      </>
      )}
    </WidgetCard>
  );
}

function Stat({
  k,
  v,
  first,
}: {
  k: string;
  v: React.ReactNode;
  first?: boolean;
}) {
  return (
    <div
      className={
        "flex justify-between py-1 " +
        (first ? "border-t" : "border-t border-dashed")
      }
    >
      <span className="text-muted-foreground">{k}</span>
      <span className="font-medium text-foreground">{v}</span>
    </div>
  );
}
