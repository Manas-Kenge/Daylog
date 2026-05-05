/**
 * Overview · the v0.1 dashboard. Five widgets, three rows, no scroll at
 * 1280×800 (per PLAN §5):
 *   Row 1 — KpiStrip (5-up tray)
 *   Row 2 — Timeline (full-width hero)
 *   Row 3 — TopApps | TopCategories | CurrentFocus (equal-height columns)
 */

import { KpiStrip } from "@/components/widgets/KpiStrip";
import { Timeline } from "@/components/widgets/Timeline";
import { TopApps } from "@/components/widgets/TopApps";
import { TopCategories } from "@/components/widgets/TopCategories";
import { CurrentFocus } from "@/components/widgets/CurrentFocus";

export function OverviewPage() {
  return (
    <>
      <KpiStrip />
      <Timeline />
      <section className="grid min-w-0 flex-1 grid-cols-3 items-stretch gap-2.5">
        <TopApps />
        <TopCategories />
        <CurrentFocus />
      </section>
    </>
  );
}
