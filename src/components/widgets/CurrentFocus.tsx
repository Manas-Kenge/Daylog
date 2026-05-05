/**
 * Current focus session + AFK ratio. Pulls the latest event run from
 * categorized_events and the AFK summary.
 */

import { WidgetCard } from "./Card";
import { useAfkSummary, useCategorizedEvents } from "@/hooks/useAw";
import { currentSession } from "@/lib/timeline";
import { categoryLabel } from "@/lib/category-colors";
import { fmtClock, fmtDuration, fmtPercent } from "@/lib/format";

export function CurrentFocus() {
  const { data: events } = useCategorizedEvents();
  const { data: afk } = useAfkSummary();

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
      action={
        <span className="inline-flex items-center gap-[5px] text-success">
          <span className="w-[5px] h-[5px] rounded-full bg-success animate-[pulse_1.8s_ease-in-out_infinite]" />
          <span className="mono text-[10.5px] tracking-[0.13em] uppercase">live</span>
        </span>
      }
    >
      <div className="flex items-center gap-[12px]">
        <svg viewBox="0 0 64 64" className="w-[56px] h-[56px] shrink-0">
          <circle cx="32" cy="32" r={FOCUS_R} fill="none" stroke="var(--secondary)" strokeWidth="3" />
          <circle
            cx="32"
            cy="32"
            r={FOCUS_R}
            fill="none"
            stroke="var(--brand-coral)"
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
            fontFamily="Geist Mono, monospace"
            fontSize="11"
            fontWeight="600"
            fill="currentColor"
          >
            {sessionMin}m
          </text>
        </svg>
        <div className="flex flex-col gap-[2px] min-w-0">
          <div className="mono text-[15px] font-semibold tracking-tight">
            {session?.app || "—"}
          </div>
          <div className="text-muted-foreground text-[11.5px] truncate max-w-[220px]">
            {session?.title || "no recent activity"}
          </div>
        </div>
      </div>

      <div className="mt-[12px]">
        <Stat k="started" v={session ? <span className="mono">{fmtClock(session.start)}</span> : "—"} first />
        <Stat k="window switches" v={<span className="mono">{switchesInSession || "—"}</span>} />
        <Stat
          k="category"
          v={session ? <span>{categoryLabel(session.category)}</span> : "—"}
        />
      </div>

      <div className="mt-[14px] pt-[12px] border-t border-border flex gap-[14px] items-center">
        <svg viewBox="0 0 64 64" className="w-[64px] h-[64px] shrink-0">
          <circle cx="32" cy="32" r={AFK_R} fill="none" stroke="var(--secondary)" strokeWidth="5" />
          <circle
            cx="32"
            cy="32"
            r={AFK_R}
            fill="none"
            stroke="var(--success)"
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
            fontFamily="Geist Mono, monospace"
            fontSize="11"
            fontWeight="600"
            fill="currentColor"
          >
            {fmtPercent(ratio)}
          </text>
        </svg>
        <div className="flex flex-col gap-[4px] flex-1 min-w-0">
          <div className="flex justify-between text-[11.5px]">
            <span className="text-muted-foreground">active</span>
            <span className="mono font-medium">{fmtDuration(afk?.active_seconds ?? 0)}</span>
          </div>
          <div className="flex justify-between text-[11.5px]">
            <span className="text-muted-foreground">afk</span>
            <span className="mono font-medium">{fmtDuration(afk?.afk_seconds ?? 0)}</span>
          </div>
          <div className="flex justify-between text-[11.5px]">
            <span className="text-muted-foreground">tracked</span>
            <span className="mono font-medium">
              {fmtDuration((afk?.active_seconds ?? 0) + (afk?.afk_seconds ?? 0))}
            </span>
          </div>
          <div className="flex h-[5px] rounded-[3px] overflow-hidden bg-secondary mt-[4px]">
            <div className="h-full bg-success" style={{ width: `${(ratio * 100).toFixed(1)}%` }} />
            <div className="h-full bg-accent" style={{ width: `${((1 - ratio) * 100).toFixed(1)}%` }} />
          </div>
        </div>
      </div>
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
        "flex justify-between py-[5px] text-[11.5px] " +
        (first ? "border-t border-border" : "border-t border-dashed border-border")
      }
    >
      <span className="text-muted-foreground">{k}</span>
      <span className="text-foreground font-medium">{v}</span>
    </div>
  );
}
