/**
 * Overview · v0.1 dashboard.
 *
 *   Row 1 — Single row: Timeline (2fr) · 3 KPI cards (1fr each)
 *           [Active/AFK · Best Window · Longest stretch].
 *   Row 2 — Two-column grid:
 *           ┌ left (1.6fr): TopApps | TopCategories · HourlyDistribution
 *           └ right (1fr) : WeekHeatmap · WebPanel
 */

import { KpiStrip } from "@/components/widgets/KpiStrip";
import { Timeline } from "@/components/widgets/Timeline";
import { TopApps } from "@/components/widgets/TopApps";
import { TopCategories } from "@/components/widgets/TopCategories";
import { WebPanel } from "@/components/widgets/WebPanel";
import { WeekHeatmap } from "@/components/widgets/WeekHeatmap";
import { HourlyDistribution } from "@/components/widgets/HourlyDistribution";

export function OverviewPage() {
  return (
    <>
      <div className="grid min-w-0 grid-cols-[minmax(0,2fr)_repeat(3,minmax(0,1fr))] gap-2.5">
        <Timeline />
        <KpiStrip />
      </div>
      <div className="grid min-w-0 grid-cols-[minmax(0,1.6fr)_minmax(0,1fr)] gap-2.5">
        <div className="flex min-w-0 flex-col gap-2.5">
          <section className="grid min-w-0 grid-cols-2 gap-2.5">
            <TopApps />
            <TopCategories />
          </section>
          <HourlyDistribution />
        </div>
        <div className="flex min-w-0 flex-col gap-2.5">
          <WeekHeatmap />
          <WebPanel />
        </div>
      </div>
    </>
  );
}
