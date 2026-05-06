/**
 * Top web domains panel. Lives in the Overview right rail and is reused
 * on Week / Month pages with a `rangeOverride` (last 7 / 30 days).
 *
 * When no aw-watcher-web bucket is detected, the card stays in place and
 * shows an install hint instead of collapsing the layout.
 */

import { ListBody, ListRow, WidgetCard } from "./Card";
import { Skeleton } from "@/components/ui/skeleton";
import { useHasWebWatcher, useTopDomains } from "@/hooks/useAw";
import { fmtDuration } from "@/lib/format";
import type { TimeRange } from "@/lib/aw-types";

const TAKE = 6;

interface WebPanelProps {
  rangeOverride?: TimeRange;
  title?: string;
  description?: string;
}

export function WebPanel({ rangeOverride, title, description }: WebPanelProps) {
  const { data: has } = useHasWebWatcher();
  const { data: domains, isLoading } = useTopDomains(rangeOverride);

  const cardTitle = title ?? "Top domains";

  if (has === false) {
    return (
      <WidgetCard title={cardTitle} description="Web activity">
        <div className="flex h-full flex-col items-center justify-center py-6 text-center text-muted-foreground">
          No web watcher detected.
          <span className="mt-1 text-[0.625rem] leading-relaxed">
            Install the Firefox or Chrome extension to track domains and URLs.
          </span>
        </div>
      </WidgetCard>
    );
  }

  const rows = (domains ?? []).slice(0, TAKE);
  const total = rows.reduce((a, r) => a + r.duration, 0);
  const loading = has === undefined || isLoading;

  return (
    <WidgetCard
      title={cardTitle}
      description={
        description ?? `${rows.length} domains · ${fmtDuration(total)}`
      }
    >
      {loading ? (
        <ListBody>
          {Array.from({ length: TAKE }, (_, i) => (
            <ListRow key={i} cols="9px_1fr_70px_60px">
              <Skeleton className="size-2 rounded-sm" />
              <Skeleton className="h-3 w-3/4" />
              <Skeleton className="h-3 w-full" />
              <Skeleton className="h-3 w-10 justify-self-end" />
            </ListRow>
          ))}
        </ListBody>
      ) : rows.length === 0 ? (
        <div className="py-6 text-center text-muted-foreground">no data</div>
      ) : (
        <ListBody>
          {rows.map((d) => {
            const pct = total > 0 ? (d.duration / total) * 100 : 0;
            return (
              <ListRow key={d.data.$domain} cols="9px_1fr_70px_60px">
                <span
                  className="size-2 rounded-sm"
                  style={{ background: "var(--chart-4)" }}
                />
                <span className="truncate font-medium">{d.data.$domain}</span>
                <span className="block h-[3px] overflow-hidden rounded-sm bg-background/50">
                  <span
                    className="block h-full"
                    style={{
                      width: `${pct.toFixed(1)}%`,
                      background: "var(--chart-4)",
                    }}
                  />
                </span>
                <span className="text-right font-mono tabular-nums text-muted-foreground">
                  {fmtDuration(d.duration)}
                </span>
              </ListRow>
            );
          })}
        </ListBody>
      )}
    </WidgetCard>
  );
}
