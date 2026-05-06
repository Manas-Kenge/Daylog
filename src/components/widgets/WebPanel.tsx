/**
 * Top web domains and URLs. Hidden when no aw-watcher-web bucket exists.
 */

import { ListBody, ListRow, WidgetCard } from "./Card";
import { Skeleton } from "@/components/ui/skeleton";
import { useHasWebWatcher, useTopDomains, useTopUrls } from "@/hooks/useAw";
import { fmtDuration } from "@/lib/format";

const TAKE = 5;

interface Row {
  key: string;
  name: string;
  duration: number;
}

export function WebPanel() {
  const { data: has } = useHasWebWatcher();
  const { data: domains, isLoading: domainsLoading } = useTopDomains();
  const { data: urls, isLoading: urlsLoading } = useTopUrls();

  if (has === false) return null;

  const domainRows: Row[] = (domains ?? []).slice(0, TAKE).map((d) => ({
    key: d.data.$domain,
    name: d.data.$domain,
    duration: d.duration,
  }));
  const urlRows: Row[] = (urls ?? []).slice(0, TAKE).map((u) => ({
    key: u.data.url,
    name: u.data.url,
    duration: u.duration,
  }));

  return (
    <WidgetCard
      title="Web · domains & URLs"
      description="From aw-watcher-web"
    >
      {has === undefined ? (
        <div className="py-4 text-center text-muted-foreground">
          checking for web watcher…
        </div>
      ) : (
        <div className="grid grid-cols-2 gap-2.5">
          <Section label="Top domains" rows={domainRows} loading={domainsLoading} />
          <Section label="Top URLs" rows={urlRows} loading={urlsLoading} />
        </div>
      )}
    </WidgetCard>
  );
}

function Section({
  label,
  rows,
  loading,
}: {
  label: string;
  rows: Row[];
  loading?: boolean;
}) {
  return (
    <div>
      <div className="mb-1 px-1.5 font-medium uppercase tracking-wider text-muted-foreground">
        {label}
      </div>
      {loading ? (
        <ListBody>
          {Array.from({ length: 5 }, (_, i) => (
            <ListRow key={i} cols="9px_1fr_60px_50px">
              <Skeleton className="size-2 rounded-sm" />
              <Skeleton className="h-3 w-3/4" />
              <Skeleton className="h-3 w-full" />
              <Skeleton className="h-3 w-10 justify-self-end" />
            </ListRow>
          ))}
        </ListBody>
      ) : rows.length === 0 ? (
        <div className="px-1.5 py-2 text-muted-foreground">no data</div>
      ) : (
        <ListBody>
          {rows.map((r) => {
            const total = rows.reduce((a, x) => a + x.duration, 0) || 1;
            const pct = (r.duration / total) * 100;
            return (
              <ListRow key={r.key} cols="9px_1fr_60px_50px">
                <span className="size-2 rounded-sm bg-[var(--chart-4)]" />
                <span className="truncate font-medium" title={r.name}>
                  {r.name}
                </span>
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
                  {fmtDuration(r.duration)}
                </span>
              </ListRow>
            );
          })}
        </ListBody>
      )}
    </div>
  );
}
