/**
 * Overview · the v0.1 dashboard, post-CEO-review (PLAN.md §1.0).
 *
 *   Row 1 — KpiStrip (5 discovery cards: Active · Best Window · Longest ·
 *           Cadence · Pattern shift)
 *   Row 2 — Timeline as visual hero, ~50% of vertical space
 *   Row 3 — TopApps | TopCategories | NotableToday
 *
 * CurrentFocus is no longer on Overview — ambient widgets belong on
 * ambient surfaces (mini-window, v0.2-roadmap GNOME applet).
 */

import { KpiStrip } from "@/components/widgets/KpiStrip";
import { Timeline } from "@/components/widgets/Timeline";
import { TopApps } from "@/components/widgets/TopApps";
import { TopCategories } from "@/components/widgets/TopCategories";
import { NotableToday } from "@/components/widgets/NotableToday";

export function OverviewPage() {
  return (
    <>
      <KpiStrip />
      <div className="min-h-0 flex-[1.6] flex flex-col">
        <Timeline />
      </div>
      <section className="grid min-w-0 grid-cols-3 items-start gap-2.5">
        <TopApps />
        <TopCategories />
        <NotableToday />
      </section>
    </>
  );
}
