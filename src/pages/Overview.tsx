/**
 * Overview · the original dashboard composition. Each row composes one or
 * more widgets at full width.
 */

import { KpiStrip } from "@/components/widgets/KpiStrip";
import { Timeline } from "@/components/widgets/Timeline";
import { HourlyDistribution } from "@/components/widgets/HourlyDistribution";
import { TopApps } from "@/components/widgets/TopApps";
import { TopCategories } from "@/components/widgets/TopCategories";
import { WeekChart } from "@/components/widgets/WeekChart";
import { CurrentFocus } from "@/components/widgets/CurrentFocus";
import { WebPanel } from "@/components/widgets/WebPanel";
import { ActivityLog } from "@/components/widgets/ActivityLog";

export function OverviewPage() {
  return (
    <>
      <KpiStrip />
      <Timeline />

      <section className="grid grid-cols-[1.3fr_1fr_1fr] gap-[10px] min-w-0">
        <HourlyDistribution />
        <TopApps />
        <TopCategories />
      </section>

      <section className="grid grid-cols-[1.6fr_1fr] gap-[10px] min-w-0">
        <WeekChart />
        <CurrentFocus />
      </section>

      <section className="grid grid-cols-[1.6fr_1fr] gap-[10px] min-w-0">
        <WebPanel />
        <ActivityLog />
      </section>
    </>
  );
}
