import { useEffect } from "react";
import { Topbar } from "@/components/layout/Topbar";
import { OverviewPage } from "@/pages/Overview";
import { WeekPage } from "@/pages/WeekPage";
import { MonthPage } from "@/pages/MonthPage";
import { Wizard } from "@/pages/Wizard";
import { usePage, type PageId } from "@/context/PageContext";
import { useFirstLaunch } from "@/hooks/useFirstLaunch";
import { useHotkey } from "@/hooks/useHotkey";

function App() {
  // Lock dark theme for now; theme-following is a later concern.
  useEffect(() => {
    document.documentElement.classList.add("dark");
  }, []);

  const firstLaunch = useFirstLaunch();
  const { page, back } = usePage();

  useHotkey({ key: "Escape", preventDefault: false, skipInInputs: true }, () => {
    if (page !== "overview") back();
  });

  if (firstLaunch.isLoading) {
    return (
      <div className="flex h-screen w-screen items-center justify-center bg-background">
        <span
          aria-hidden
          className="block h-5 w-5 animate-spin rounded-full border-2 border-current border-t-transparent text-muted-foreground"
        />
      </div>
    );
  }

  if (!firstLaunch.complete) {
    return <Wizard onComplete={firstLaunch.markComplete} />;
  }

  return (
    <div className="grid h-screen w-screen grid-rows-[auto_1fr] overflow-hidden bg-background">
      <Topbar />
      <main className="flex min-h-0 min-w-0 flex-col gap-2.5 overflow-y-auto px-3.5 pb-5 pt-3">
        <PageOutlet page={page} />
      </main>
    </div>
  );
}

function PageOutlet({ page }: { page: PageId }) {
  switch (page) {
    case "overview": return <OverviewPage />;
    case "week":     return <WeekPage />;
    case "month":    return <MonthPage />;
  }
}

export default App;
