/**
 * Pulse left sidebar. Active id and onSelect are owned by App.tsx so the
 * main pane can swap content per nav item.
 */

import type { ReactNode } from "react";
import { cn } from "@/lib/utils";
import type { NavId } from "@/lib/nav";
import { useHasWebWatcher, useInfo, useBuckets } from "@/hooks/useAw";

interface NavItem {
  id: NavId;
  label: string;
  icon: ReactNode;
}

interface NavGroup {
  label: string;
  items: NavItem[];
}

const Icon = ({ d }: { d: string }) => (
  <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4">
    {d.split("|").map((seg, i) => (
      <path key={i} d={seg} />
    ))}
  </svg>
);

const NAV: NavGroup[] = [
  {
    label: "Tracking",
    items: [
      { id: "overview",   label: "Overview",     icon: <Icon d="M2 2h5v5H2z|M9 2h5v5H9z|M2 9h5v5H9z|M9 9h5v5H9z" /> },
      { id: "apps",       label: "Apps",         icon: <Icon d="M2 3h12v10H2z|M2 6h12" /> },
      { id: "categories", label: "Categories",   icon: <Icon d="M5 5a3 3 0 1 0 0 0|M11 11a3 3 0 1 0 0 0|M7.5 7.5l1 1" /> },
      { id: "web",        label: "Web",          icon: <Icon d="M8 2a6 6 0 1 0 0 12 6 6 0 0 0 0-12|M2 8h12|M8 2c2 2 2 10 0 12|M8 2c-2 2-2 10 0 12" /> },
      { id: "activity",   label: "Activity log", icon: <Icon d="M3 3h10v10H3z|M5 6h6|M5 8h6|M5 10h4" /> },
    ],
  },
  {
    label: "Insights",
    items: [
      { id: "hourly",  label: "Hourly patterns", icon: <Icon d="M2 13h12|M3 13V8|M6 13V5|M9 13V9|M12 13V3" /> },
      { id: "weekly",  label: "Weekly trends",   icon: <Icon d="M2 3h12v10H2z|M2 6h12|M5 3v10|M8 3v10|M11 3v10" /> },
      { id: "compare", label: "Compare days",    icon: <Icon d="M2 8c0-3 3-5 6-5s6 2 6 5-3 5-6 5-6-2-6-5z|M8 3v10" /> },
    ],
  },
  {
    label: "Settings",
    items: [
      { id: "settings-tracking",   label: "Tracking",   icon: <Icon d="M8 5.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5|M8 2v2|M8 12v2|M2 8h2|M12 8h2|M3.5 3.5l1.4 1.4|M11.1 11.1l1.4 1.4|M3.5 12.5l1.4-1.4|M11.1 4.9l1.4-1.4" /> },
      { id: "settings-categories", label: "Categories", icon: <Icon d="M3 3h10v3H3z|M3 7h10v3H3z|M3 11h7v2H3z" /> },
      { id: "settings-general",    label: "General",    icon: <Icon d="M8 2a6 6 0 1 0 0 12 6 6 0 0 0 0-12|M8 5v3l2 2" /> },
    ],
  },
];

export function Sidebar({
  active,
  onSelect,
}: {
  active: NavId;
  onSelect: (id: NavId) => void;
}) {
  const { data: hasWatcher } = useHasWebWatcher();
  const { data: info } = useInfo();
  const { data: buckets } = useBuckets();

  return (
    <aside className="bg-sidebar border-r border-sidebar-border flex flex-col min-h-0">
      <div className="px-[14px] py-[12px] flex items-center justify-between border-b border-sidebar-border">
        <div className="flex items-center gap-[9px] min-w-0">
          <div className="w-[22px] h-[22px] rounded-[var(--radius)] bg-sidebar-accent-brighter border border-sidebar-border relative shrink-0">
            <span className="absolute inset-0 m-auto w-[7px] h-[7px] rounded-full bg-brand-coral shadow-[0_0_8px_var(--brand-coral)]" />
          </div>
          <span className="font-semibold text-[13.5px] tracking-tight">Pulse</span>
          <span className="mono text-[10px] text-muted-foreground px-[6px] py-[2px] border border-border rounded-[var(--radius-sm)]">
            v0.1
          </span>
        </div>
        <button
          type="button"
          aria-label="collapse sidebar"
          className="text-muted-foreground hover:text-foreground hover:bg-sidebar-accent rounded-[var(--radius-sm)] px-[6px] text-[16px] leading-none cursor-pointer"
        >
          ‹
        </button>
      </div>

      <nav className="flex-1 min-h-0 overflow-y-auto px-[8px] pt-[12px] pb-[16px]">
        {NAV.map((group, gi) => (
          <div key={group.label} className={gi === 0 ? "" : "mt-[12px]"}>
            <div className="px-[10px] pt-[6px] pb-[4px] text-[10px] tracking-[0.13em] uppercase text-muted-foreground font-medium">
              {group.label}
            </div>
            {group.items.map((item) => (
              <SidebarItem
                key={item.id}
                item={item}
                active={active === item.id}
                onClick={() => onSelect(item.id)}
                badge={
                  item.id === "web" && hasWatcher
                    ? <span className="mono text-[9px] text-success px-[5px] py-[1px] border border-border rounded-[3px]">live</span>
                    : null
                }
              />
            ))}
          </div>
        ))}
      </nav>

      <div className="border-t border-sidebar-border px-[10px] pt-[10px] pb-[12px] flex flex-col gap-[6px] text-[11px] text-muted-foreground">
        <div className="flex items-center gap-[8px] px-[8px] py-[6px] border border-border rounded-[var(--radius)] bg-sidebar-accent">
          <span className="w-[7px] h-[7px] rounded-full bg-success shadow-[0_0_8px_var(--success)] shrink-0 animate-[pulse_1.8s_ease-in-out_infinite]" />
          <span className="flex flex-col leading-[1.25] flex-1 min-w-0">
            <span className="mono text-foreground text-[11px] truncate">
              {info ? `aw-server v${info.version}` : "aw-server …"}
            </span>
            <span className="text-[10px] text-muted-foreground">
              {info ? "external · :5600" : "connecting…"}
            </span>
          </span>
        </div>
        <FootRow k="host" v={info?.hostname ?? "—"} />
        <FootRow k="buckets" v={buckets ? String(buckets.length) : "—"} />
      </div>
    </aside>
  );
}

function SidebarItem({
  item,
  active,
  onClick,
  badge,
}: {
  item: NavItem;
  active: boolean;
  onClick: () => void;
  badge?: ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "w-full grid grid-cols-[16px_1fr_auto] gap-[10px] items-center px-[10px] py-[6px] rounded-[var(--radius)] text-[12.5px] cursor-pointer transition-colors duration-100 ease-out text-left",
        active
          ? "bg-sidebar-accent text-foreground shadow-[inset_2px_0_0_var(--brand-coral)]"
          : "text-muted-foreground hover:bg-sidebar-accent hover:text-foreground",
      )}
    >
      <span className="inline-flex items-center justify-center w-[14px] h-[14px]">
        {item.icon}
      </span>
      <span className="truncate">{item.label}</span>
      <span>{badge}</span>
    </button>
  );
}

function FootRow({ k, v }: { k: string; v: string }) {
  return (
    <div className="flex justify-between items-center px-[4px] text-[10.5px]">
      <span className="text-muted-foreground">{k}</span>
      <span className="text-foreground mono">{v}</span>
    </div>
  );
}
