/**
 * Web · top domains and top URLs side-by-side. Promoted from the
 * dashboard widget to a palette-reachable detail page (PLAN §5).
 *
 * Hidden gracefully when no aw-watcher-web bucket exists.
 */

import { ListBody, ListRow, WidgetCard } from "@/components/widgets/Card";
import { useHasWebWatcher, useTopDomains, useTopUrls } from "@/hooks/useAw";
import { fmtDuration } from "@/lib/format";

export function WebPage() {
  const { data: has } = useHasWebWatcher();
  const { data: domains } = useTopDomains();
  const { data: urls } = useTopUrls();

  if (has === false) {
    return (
      <WidgetCard title="Web" description="Top domains and URLs">
        <div className="py-10 text-center leading-relaxed text-muted-foreground">
          No <code className="font-mono tabular-nums">aw-watcher-web</code> bucket detected.
          <br />
          Install the Firefox or Chrome extension to start tracking web activity.
        </div>
      </WidgetCard>
    );
  }

  const domainTotal = (domains ?? []).reduce((a, d) => a + d.duration, 0);
  const urlTotal = (urls ?? []).reduce((a, u) => a + u.duration, 0);

  return (
    <section className="grid grid-cols-2 gap-2.5">
      <WidgetCard
        title="Top domains"
        description={`${domains?.length ?? 0} unique domains · ${fmtDuration(domainTotal)} total`}
      >
        {!domains || domains.length === 0 ? (
          <Empty>no data</Empty>
        ) : (
          <ListBody>
            {domains.map((d) => {
              const pct = domainTotal > 0 ? (d.duration / domainTotal) * 100 : 0;
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

      <WidgetCard
        title="Top URLs"
        description={`${urls?.length ?? 0} unique URLs · ${fmtDuration(urlTotal)} total`}
      >
        {!urls || urls.length === 0 ? (
          <Empty>no data</Empty>
        ) : (
          <ListBody>
            {urls.map((u) => {
              const pct = urlTotal > 0 ? (u.duration / urlTotal) * 100 : 0;
              return (
                <ListRow key={u.data.url} cols="9px_1fr_70px_60px">
                  <span
                    className="size-2 rounded-sm"
                    style={{ background: "var(--chart-4)" }}
                  />
                  <span className="truncate font-medium" title={u.data.url}>
                    {u.data.url}
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
                    {fmtDuration(u.duration)}
                  </span>
                </ListRow>
              );
            })}
          </ListBody>
        )}
      </WidgetCard>
    </section>
  );
}

function Empty({ children }: { children: React.ReactNode }) {
  return (
    <div className="py-6 text-center text-muted-foreground">{children}</div>
  );
}
