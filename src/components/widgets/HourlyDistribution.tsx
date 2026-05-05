/**
 * 24-hour distribution. Today as bars, yesterday as a dashed overlay line.
 */

import {
  Bar,
  CartesianGrid,
  ComposedChart,
  Line,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { WidgetCard } from "./Card";
import { Skeleton } from "@/components/ui/skeleton";
import { useHourlyTodayVsYesterday } from "@/hooks/useAw";
import { fmtDuration } from "@/lib/format";

interface ChartRow {
  hour: number;
  today: number;
  yesterday: number;
}

export function HourlyDistribution() {
  const { today, yesterday } = useHourlyTodayVsYesterday();

  const data: ChartRow[] = Array.from({ length: 24 }, (_, h) => ({
    hour: h,
    today: today.data?.[h]?.duration ?? 0,
    yesterday: yesterday.data?.[h]?.duration ?? 0,
  }));

  return (
    <WidgetCard
      title="Hourly distribution"
      description={
        <span>
          today <span style={{ color: "var(--chart-1)" }}>●</span> · yesterday{" "}
          <span className="text-muted-foreground">○</span>
        </span>
      }
    >
      <div className="h-[120px] w-full">
        {today.isLoading ? (
          <Skeleton className="h-full w-full rounded-sm" />
        ) : (
        <ResponsiveContainer width="100%" height="100%">
          <ComposedChart
            data={data}
            margin={{ top: 4, right: 4, left: -20, bottom: 0 }}
            barCategoryGap={2}
          >
            <CartesianGrid
              vertical={false}
              stroke="var(--border)"
              strokeDasharray="2 4"
              opacity={0.4}
            />
            <XAxis
              dataKey="hour"
              tick={{ fontSize: 10, fill: "var(--muted-foreground)" }}
              tickFormatter={(h: number) => (h % 3 === 0 ? String(h) : "")}
              axisLine={false}
              tickLine={false}
              interval={0}
            />
            <YAxis
              tick={{ fontSize: 10, fill: "var(--muted-foreground)" }}
              tickFormatter={(v: number) => (v >= 60 ? `${Math.round(v / 60)}m` : "")}
              axisLine={false}
              tickLine={false}
              width={32}
            />
            <Tooltip
              cursor={{ fill: "var(--accent)", opacity: 0.4 }}
              content={({ active, payload }) => {
                if (!active || !payload?.length) return null;
                const row = payload[0].payload as ChartRow;
                return (
                  <div className="rounded-md border bg-popover p-2 shadow-lg">
                    <div className="mb-1 font-mono tabular-nums font-medium text-foreground">
                      {String(row.hour).padStart(2, "0")}:00
                    </div>
                    <div className="flex items-center gap-2">
                      <span
                        className="size-2 rounded-full"
                        style={{ background: "var(--chart-1)" }}
                      />
                      <span className="text-muted-foreground">today</span>
                      <span className="ml-auto font-mono tabular-nums font-medium">
                        {fmtDuration(row.today)}
                      </span>
                    </div>
                    <div className="mt-0.5 flex items-center gap-2">
                      <span className="size-2 rounded-full bg-muted-foreground" />
                      <span className="text-muted-foreground">yest</span>
                      <span className="ml-auto font-mono tabular-nums font-medium">
                        {fmtDuration(row.yesterday)}
                      </span>
                    </div>
                  </div>
                );
              }}
            />
            <Bar
              dataKey="today"
              fill="var(--chart-1)"
              radius={[2, 2, 0, 0]}
              isAnimationActive={false}
            />
            <Line
              type="monotone"
              dataKey="yesterday"
              stroke="var(--muted-foreground)"
              strokeWidth={1.2}
              strokeDasharray="3 3"
              dot={false}
              opacity={0.6}
              isAnimationActive={false}
            />
          </ComposedChart>
        </ResponsiveContainer>
        )}
      </div>
    </WidgetCard>
  );
}
