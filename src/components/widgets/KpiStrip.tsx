/**
 * 5-up KPI tray. Each cell pairs a sparkline with a [icon · value · delta] row.
 */

import { Area, AreaChart, ResponsiveContainer } from "recharts";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  Activity03Icon,
  ArrowLeftRightIcon,
  DashboardSquare01Icon,
  PercentSquareIcon,
  PieChartIcon,
} from "@hugeicons/core-free-icons";
import type { IconSvgElement } from "@hugeicons/react";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
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

interface KpiCardProps {
  icon: IconSvgElement;
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

  if (loading) {
    return (
      <Card size="sm" className="gap-0 overflow-hidden py-0">
        <Skeleton className="h-[68px] rounded-none" />
        <div className="flex items-center gap-2.5 px-3 py-2.5">
          <Skeleton className="size-7 rounded-sm" />
          <div className="min-w-0 flex-1 flex flex-col gap-1.5">
            <Skeleton className="h-4 w-3/4" />
            <Skeleton className="h-3 w-1/2" />
          </div>
        </div>
      </Card>
    );
  }

  return (
    <Card size="sm" className="gap-0 overflow-hidden py-0">
      <div className="-mb-px h-[68px]">
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
          <div className="size-full" />
        )}
      </div>

      <div className="flex items-center gap-2.5 px-3 py-2.5">
        <div className="flex size-7 shrink-0 items-center justify-center rounded-sm border bg-secondary text-muted-foreground">
          <HugeiconsIcon icon={icon} size={14} />
        </div>
        <div className="min-w-0 flex-1">
          <div className="truncate font-mono tabular-nums text-lg font-semibold leading-tight tracking-tight text-foreground">
            {value}
          </div>
          <div className="truncate text-muted-foreground">{label}</div>
        </div>
        {delta && (
          <Badge
            variant={
              delta.tone === "down"
                ? "destructive"
                : delta.tone === "up"
                  ? "outline"
                  : "secondary"
            }
            className="font-mono tabular-nums"
          >
            {delta.text}
          </Badge>
        )}
      </div>
    </Card>
  );
}

export function KpiStrip() {
  const { today: afkToday, yesterday: afkYest } = useAfkTodayVsYesterday();
  const { data: hourly, isLoading: hourlyLoading } = useHourly();
  const { data: cats, isLoading: catsLoading } = useTopCategories();
  const { data: apps, isLoading: appsLoading } = useTopApps();
  const { data: categorized, isLoading: catzdLoading } = useCategorizedEvents();

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
    <section className="grid grid-cols-5 gap-1.5 rounded-xl bg-secondary p-1.5">
      <KpiCard
        icon={Activity03Icon}
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
        loading={afkToday.isLoading || hourlyLoading}
      />
      <KpiCard
        icon={PercentSquareIcon}
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
        icon={ArrowLeftRightIcon}
        label="Switches"
        value={String(switches)}
        spark={{ values: switchesByHour, color: "var(--chart-2)" }}
        loading={catzdLoading}
      />
      <KpiCard
        icon={DashboardSquare01Icon}
        label={`Apps · unique`}
        value={String(uniqueApps)}
        spark={{ values: uniqueByHour, color: "var(--chart-4)" }}
        loading={appsLoading}
      />
      <KpiCard
        icon={PieChartIcon}
        label={topCat ? `${fmtDuration(topCat.duration)} · ${fmtPercent(topCatPct)}` : "no data"}
        value={
          <span className="text-base">{topCat ? categoryRoot(topCat.name) : "—"}</span>
        }
        spark={{ values: activeSpark, color: "var(--chart-3)" }}
        loading={catsLoading}
      />
    </section>
  );
}
