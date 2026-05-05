/**
 * Generic "coming soon" placeholder for sidebar items that aren't built yet.
 */

import { WidgetCard } from "@/components/widgets/Card";
import { PAGE_TITLES, type NavId } from "@/lib/nav";

export function PlaceholderPage({ id }: { id: NavId }) {
  return (
    <WidgetCard
      title={PAGE_TITLES[id]}
      description="Not built yet"
    >
      <div className="text-muted-foreground text-[12px] py-[40px] text-center">
        This page is on the roadmap.
        <br />
        For now, the relevant data lives on the Overview page.
      </div>
    </WidgetCard>
  );
}
