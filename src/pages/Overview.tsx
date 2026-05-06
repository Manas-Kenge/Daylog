/**
 * Overview · the v0.1 dashboard, post-CEO-review (PLAN.md §1.0).
 *
 *   Row 1 — KpiStrip (5 discovery cards: Active · Best Window · Longest ·
 *           Cadence · Pattern shift)
 *   Row 2 — Two-column grid:
 *           ┌ left (1.6fr): Timeline hero · TopApps | TopCategories two-up
 *           └ right (1fr) : WeekHeatmap · WebPanel
 *
 * The right rail gives "this week" context permanent residency next to
 * today's data — habit visibility without a page hop. WebPanel keeps the
 * web slot but moves out of the bottom 3-up to its own rail.
 *
 * CurrentFocus is no longer on Overview — ambient widgets belong on
 * ambient surfaces (mini-window, v0.2-roadmap GNOME applet).
 *
 * NotableToday lived here previously but was demoted: its "Building baseline
 * (X/7)" empty state dominates for new users, and once seeded its anomalies
 * are rare. The web slot earns the same real estate in both states — domain
 * data when the extension is installed, an install hint when it isn't.
 */

import { KpiStrip } from "@/components/widgets/KpiStrip";
import { Timeline } from "@/components/widgets/Timeline";
import { TopApps } from "@/components/widgets/TopApps";
import { TopCategories } from "@/components/widgets/TopCategories";
import { WebPanel } from "@/components/widgets/WebPanel";
import { WeekHeatmap } from "@/components/widgets/WeekHeatmap";

export function OverviewPage() {
  return (
    <>
      <KpiStrip />
      <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,1.6fr)_minmax(0,1fr)] gap-2.5">
        <div className="flex min-h-0 min-w-0 flex-col gap-2.5">
          <div className="min-h-0 flex-[1.6]">
            <Timeline />
          </div>
          <section className="grid min-h-0 min-w-0 grid-cols-2 items-start gap-2.5">
            <TopApps />
            <TopCategories />
          </section>
        </div>
        <div className="flex min-h-0 min-w-0 flex-col gap-2.5">
          <WeekHeatmap />
          <WebPanel />
        </div>
      </div>
    </>
  );
}
