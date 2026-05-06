/**
 * Top categories · donut chart + compact legend.
 * Built on shadcn `ChartContainer` + Recharts `PieChart`.
 */

import { Cell, Pie, PieChart } from "recharts";
import { WidgetCard } from "./Card";
import { Badge } from "@/components/ui/badge";
import {
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from "@/components/ui/chart";
import { Skeleton } from "@/components/ui/skeleton";
import { useTopCategories } from "@/hooks/useAw";
import { fmtDuration } from "@/lib/format";
import { categoryColor, categoryRoot } from "@/lib/category-colors";
import type { TimeRange } from "@/lib/aw-types";

interface TopCategoriesProps {
  rangeOverride?: TimeRange;
  title?: string;
  description?: string;
}

export function TopCategories({
  rangeOverride,
  title = "Top categories",
  description = "Time per category",
}: TopCategoriesProps = {}) {
  const { data, isLoading } = useTopCategories(rangeOverride);
  const total = (data ?? []).reduce((a, c) => a + c.duration, 0);

  const rows = (data ?? []).map((cat) => ({
    key: cat.name.join("/"),
    label: cat.name[0] ?? "Uncategorized",
    sub: cat.name[1],
    duration: cat.duration,
    color: categoryColor(cat.name),
    root: categoryRoot(cat.name),
  }));

  const config: ChartConfig = Object.fromEntries(
    rows.map((r) => [r.key, { label: r.label, color: r.color }]),
  );

  return (
    <WidgetCard
      title={title}
      description={description}
      action={
        <Badge variant="outline" className="font-mono tabular-nums uppercase">
          {fmtDuration(total)} total
        </Badge>
      }
    >
      {isLoading ? (
        <div className="flex flex-col gap-3">
          <Skeleton className="mx-auto size-40 rounded-full" />
          <div className="flex flex-col gap-0.5 rounded-md bg-background p-1">
            {Array.from({ length: 5 }, (_, i) => (
              <div
                key={i}
                className="grid grid-cols-[9px_1fr_auto_auto] items-center gap-2.5 rounded-sm bg-muted/30 px-2.5 py-1.5"
              >
                <Skeleton className="size-2 rounded-sm" />
                <Skeleton className="h-3 w-1/2" />
                <Skeleton className="h-3 w-8" />
                <Skeleton className="h-3 w-12" />
              </div>
            ))}
          </div>
        </div>
      ) : rows.length === 0 ? (
        <div className="py-4 text-center text-muted-foreground">
          no categorized activity yet
        </div>
      ) : (
        <div className="flex flex-col gap-3">
          <ChartContainer
            config={config}
            className="mx-auto aspect-square h-40 w-40"
          >
            <PieChart>
              <ChartTooltip
                cursor={false}
                content={
                  <ChartTooltipContent
                    hideLabel
                    formatter={(value, name) => (
                      <div className="flex w-full items-center gap-2">
                        <span
                          className="size-2 shrink-0 rounded-sm"
                          style={{
                            background:
                              config[name as string]?.color ?? "var(--muted)",
                          }}
                        />
                        <span className="text-muted-foreground">
                          {config[name as string]?.label ?? name}
                        </span>
                        <span className="ml-auto font-mono tabular-nums font-medium text-foreground">
                          {fmtDuration(Number(value))}
                        </span>
                      </div>
                    )}
                  />
                }
              />
              <Pie
                data={rows}
                dataKey="duration"
                nameKey="key"
                innerRadius={42}
                outerRadius={72}
                strokeWidth={2}
                stroke="var(--card)"
              >
                {rows.map((r) => (
                  <Cell key={r.key} fill={r.color} />
                ))}
              </Pie>
            </PieChart>
          </ChartContainer>

          <div className="flex flex-col gap-0.5 rounded-md bg-background p-1">
            {rows.map((r) => {
              const pct = total > 0 ? (r.duration / total) * 100 : 0;
              return (
                <div
                  key={r.key}
                  className="grid grid-cols-[9px_1fr_auto_auto] items-center gap-2.5 rounded-sm bg-muted/30 px-2.5 py-1.5"
                >
                  <span
                    className="size-2 rounded-sm"
                    style={{ background: r.color }}
                  />
                  <span className="truncate font-medium">
                    {r.label}
                    {r.sub ? (
                      <span className="ml-1 font-normal text-muted-foreground">
                        / {r.sub}
                      </span>
                    ) : null}
                  </span>
                  <span className="font-mono tabular-nums text-muted-foreground">
                    {pct.toFixed(0)}%
                  </span>
                  <span className="min-w-14 text-right font-mono tabular-nums text-muted-foreground">
                    {fmtDuration(r.duration)}
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      )}
    </WidgetCard>
  );
}
