/**
 * Top web domains and URLs. Hidden when no aw-watcher-web bucket exists.
 */

import { ListBody, ListRow, WidgetCard } from "./Card";
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
  const { data: domains } = useTopDomains();
  const { data: urls } = useTopUrls();

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
      action={
        <span className="mono text-[10.5px] text-success tracking-[0.13em] uppercase">
          live
        </span>
      }
    >
      {has === undefined ? (
        <div className="text-muted-foreground text-[12px] py-[16px] text-center">
          checking for web watcher…
        </div>
      ) : (
        <div className="grid grid-cols-2 gap-[10px]">
          <Section label="Top domains" rows={domainRows} />
          <Section label="Top URLs" rows={urlRows} />
        </div>
      )}
    </WidgetCard>
  );
}

function Section({ label, rows }: { label: string; rows: Row[] }) {
  return (
    <div>
      <div className="text-[10px] tracking-[0.13em] uppercase text-muted-foreground font-medium px-[6px] mb-[4px]">
        {label}
      </div>
      {rows.length === 0 ? (
        <div className="text-muted-foreground text-[11.5px] py-[8px] px-[6px]">
          no data
        </div>
      ) : (
        <ListBody>
          {rows.map((r) => {
            const total = rows.reduce((a, x) => a + x.duration, 0) || 1;
            const pct = (r.duration / total) * 100;
            return (
              <ListRow key={r.key} cols="9px_1fr_60px_50px">
                <span className="w-[8px] h-[8px] rounded-[2px] bg-[var(--chart-4)]" />
                <span
                  className="font-medium text-[12px] truncate"
                  title={r.name}
                >
                  {r.name}
                </span>
                <span className="h-[3px] bg-background/50 rounded-[2px] overflow-hidden block">
                  <span
                    className="h-full block"
                    style={{ width: `${pct.toFixed(1)}%`, background: "var(--chart-4)" }}
                  />
                </span>
                <span className="mono text-muted-foreground text-[11.5px] text-right">
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
