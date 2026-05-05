/**
 * 5-up KPI tray. Outer container is `bg-secondary p-1.5 rounded-xl` (databuddy
 * "tray" pattern); each card has the area chart spanning the top and a row of
 * [icon · value + label · delta chip] at the bottom.
 */

import { Area, AreaChart, ResponsiveContainer } from "recharts";
import {
  useAfkTodayVsYesterday,
  useCategorizedEvents,
  useHourly,
  useTopApps,
  useTopCategories,
} from "@/hooks/useAw";
import { fmtDuration, fmtPercent } from "@/lib/format";
import { categoryRoot } from "@/lib/category-colors";
import { useId, type ReactNode } from "react";
import { cn } from "@/lib/utils";

interface KpiCardProps {
  icon: ReactNode;
  label: string;
  value: ReactNode;
  delta?: { text: string; tone: "up" | "down" | "flat" };
  spark: { values: number[]; color: string };
  loading?: boolean;
}

function KpiCard({ icon, label, value, delta, spark, loading }: KpiCardProps) {
  const gradId = useId();
  const data = spark.values.map((v, i) => ({ x: i, value: v }));
  const hasData = data.some((d) => d.value > 0);

  return (
    <div className="rounded-[var(--radius)] bg-card overflow-hidden flex flex-col">
      <div className="h-[68px] -mb-px">
        {hasData ? (
          <ResponsiveContainer width="100%" height="100%">
            <AreaChart data={data} margin={{ top: 4, right: 0, left: 0, bottom: 0 }}>
              <defs>
                <linearGradient id={gradId} x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stopColor={spark.color} stopOpacity={0.32} />
                  <stop offset="100%" stopColor={spark.color} stopOpacity={0.02} />
                </linearGradient>
              </defs>
              <Area
                type="monotone"
                dataKey="value"
                stroke={spark.color}
                strokeWidth={1.5}
                fill={`url(#${gradId})`}
                isAnimationActive={false}
              />
            </AreaChart>
          </ResponsiveContainer>
        ) : (
          <div className="h-full w-full" />
        )}
      </div>

      <div className="flex items-center gap-[10px] px-[12px] py-[10px]">
        <div className="size-[28px] rounded-[var(--radius-sm)] bg-secondary border border-border flex items-center justify-center text-muted-foreground text-[12px] shrink-0">
          {icon}
        </div>
        <div className="flex-1 min-w-0">
          <div className="mono text-[18px] font-semibold tracking-tight text-foreground leading-tight truncate">
            {loading ? "…" : value}
          </div>
          <div className="text-[11px] text-muted-foreground truncate">{label}</div>
        </div>
        {delta && (
          <div
            className={cn(
              "mono text-[10.5px] px-[7px] py-[2px] border rounded-[3px] shrink-0",
              delta.tone === "up" && "text-success border-success/30",
              delta.tone === "down" && "text-destructive border-destructive/30",
              delta.tone === "flat" && "text-muted-foreground border-border",
            )}
          >
            {delta.text}
          </div>
        )}
      </div>
    </div>
  );
}

export function KpiStrip() {
  const { today: afkToday, yesterday: afkYest } = useAfkTodayVsYesterday();
  const { data: hourly } = useHourly();
  const { data: cats } = useTopCategories();
  const { data: apps } = useTopApps();
  const { data: categorized } = useCategorizedEvents();

  // Active
  const activeSec = afkToday.data?.active_seconds ?? 0;
  const yestActiveSec = afkYest.data?.active_seconds ?? 0;
  const activeDelta = activeSec - yestActiveSec;
  const activeSpark = (hourly ?? []).map((h) => h.duration);

  // Activity %
  const ratio = afkToday.data?.active_ratio ?? 0;
  const ratioYest = afkYest.data?.active_ratio ?? 0;
  const ratioDeltaPp = (ratio - ratioYest) * 100;
  const ratioSpark = activeSpark.map((s) => Math.min(1, s / 3600));

  // Switches
  const switches = categorized?.length ?? 0;
  const switchesByHour = new Array(24).fill(0);
  for (const ev of categorized ?? []) {
    const h = new Date(ev.timestamp).getHours();
    if (h >= 0 && h < 24) switchesByHour[h] += 1;
  }

  // Apps (cumulative unique)
  const uniqueApps = apps?.length ?? 0;
  const uniqueByHour = new Array(24).fill(0);
  if (categorized) {
    const seen = new Set<string>();
    const sorted = [...categorized].sort((a, b) =>
      a.timestamp < b.timestamp ? -1 : 1,
    );
    let pointer = 0;
    for (let h = 0; h < 24; h++) {
      while (
        pointer < sorted.length &&
        new Date(sorted[pointer].timestamp).getHours() <= h
      ) {
        const app = (sorted[pointer].data as { app?: string })?.app;
        if (app) seen.add(app);
        pointer++;
      }
      uniqueByHour[h] = seen.size;
    }
  }

  // Top category
  const topCat = cats?.[0];
  const totalCatsSec = (cats ?? []).reduce((a, c) => a + c.duration, 0);
  const topCatPct = topCat && totalCatsSec > 0 ? topCat.duration / totalCatsSec : 0;

  return (
    <section className="grid grid-cols-5 gap-[6px] rounded-[var(--radius-xl)] bg-secondary p-[6px]">
      <KpiCard
        icon="●"
        label={`vs yest · ${activeDelta >= 0 ? "+" : "−"}${fmtDuration(Math.abs(activeDelta))}`}
        value={fmtDuration(activeSec)}
        delta={
          afkYest.data
            ? {
                text: `${activeDelta >= 0 ? "↑" : "↓"} ${fmtDuration(Math.abs(activeDelta))}`,
                tone: activeDelta > 0 ? "up" : activeDelta < 0 ? "down" : "flat",
              }
            : undefined
        }
        spark={{ values: activeSpark, color: "var(--chart-1)" }}
        loading={afkToday.isLoading}
      />
      <KpiCard
        icon="%"
        label="Activity"
        value={fmtPercent(ratio)}
        delta={
          afkYest.data
            ? {
                text: `${ratioDeltaPp >= 0 ? "↑" : "↓"} ${Math.abs(ratioDeltaPp).toFixed(0)}pp`,
                tone: ratioDeltaPp > 0 ? "up" : ratioDeltaPp < 0 ? "down" : "flat",
              }
            : undefined
        }
        spark={{ values: ratioSpark, color: "var(--chart-5)" }}
        loading={afkToday.isLoading}
      />
      <KpiCard
        icon="⇄"
        label="Switches"
        value={String(switches)}
        spark={{ values: switchesByHour, color: "var(--chart-2)" }}
      />
      <KpiCard
        icon="▦"
        label={`Apps · unique`}
        value={String(uniqueApps)}
        spark={{ values: uniqueByHour, color: "var(--chart-4)" }}
      />
      <KpiCard
        icon="▲"
        label={topCat ? `${fmtDuration(topCat.duration)} · ${fmtPercent(topCatPct)}` : "no data"}
        value={
          <span className="text-[15px]">{topCat ? categoryRoot(topCat.name) : "—"}</span>
        }
        spark={{ values: activeSpark, color: "var(--chart-3)" }}
      />
    </section>
  );
}
