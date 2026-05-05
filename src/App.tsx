import { useEffect, useState } from "react";
import { Sidebar } from "@/components/layout/Sidebar";
import { Topbar } from "@/components/layout/Topbar";
import { OverviewPage } from "@/pages/Overview";
import { AppsPage } from "@/pages/AppsPage";
import { ActivityLogPage } from "@/pages/ActivityLogPage";
import { HourlyPatternsPage } from "@/pages/HourlyPatternsPage";
import { PlaceholderPage } from "@/pages/Placeholder";
import { PAGE_TITLES, type NavId } from "@/lib/nav";

function App() {
  // Lock dark theme for now; theme-following is a later concern.
  useEffect(() => {
    document.documentElement.classList.add("dark");
  }, []);

  const [view, setView] = useState<NavId>("overview");

  return (
    <div className="grid grid-cols-[232px_1fr] h-screen w-screen overflow-hidden">
      <Sidebar active={view} onSelect={setView} />
      <main className="grid grid-rows-[auto_1fr] min-w-0 min-h-0 bg-background">
        <Topbar pageTitle={PAGE_TITLES[view]} />
        <div className="overflow-y-auto px-[14px] pt-[12px] pb-[20px] flex flex-col gap-[10px]">
          {view === "overview" && <OverviewPage />}
          {view === "apps" && <AppsPage />}
          {view === "activity" && <ActivityLogPage />}
          {view === "hourly" && <HourlyPatternsPage />}
          {view !== "overview" &&
            view !== "apps" &&
            view !== "activity" &&
            view !== "hourly" && <PlaceholderPage id={view} />}
        </div>
      </main>
    </div>
  );
}

export default App;
