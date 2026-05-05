/**
 * Pulse command palette. Built on cmdk. Mounted once at App root.
 *
 * Hotkeys: ⌘K / Ctrl+K to open. Esc dismisses. ? opens shortcut help.
 */

import { Command } from "cmdk";
import { useEffect, useState } from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  DashboardSquare01Icon,
  PieChartIcon,
} from "@hugeicons/core-free-icons";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { useHotkey } from "@/hooks/useHotkey";
import { usePage } from "@/context/PageContext";
import { useRange } from "@/context/RangeContext";
import { useTopApps, useTopCategories } from "@/hooks/useAw";
import { categoryRoot } from "@/lib/category-colors";
import { fmtDuration } from "@/lib/format";
import {
  STATIC_COMMANDS,
  type PaletteCommand,
} from "./commands";

const DYNAMIC_TAKE = 20;

const GROUP_HEADING_CLASS =
  "[&_[cmdk-group-heading]]:px-3 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:text-[0.625rem] [&_[cmdk-group-heading]]:font-medium [&_[cmdk-group-heading]]:uppercase [&_[cmdk-group-heading]]:tracking-wider [&_[cmdk-group-heading]]:text-muted-foreground";

const ITEM_CLASS =
  "mx-1.5 flex cursor-pointer items-center gap-2.5 rounded-sm px-3 py-1.5 data-[selected=true]:bg-accent data-[selected=true]:text-foreground";

export function CommandPalette() {
  const [open, setOpen] = useState(false);
  const [showShortcuts, setShowShortcuts] = useState(false);
  const [search, setSearch] = useState("");

  const { setPreset } = useRange();
  const { push } = usePage();

  const { data: apps } = useTopApps();
  const { data: cats } = useTopCategories();

  useHotkey({ key: "k", meta: true }, () => setOpen((v) => !v));
  useHotkey({ key: "k", ctrl: true }, () => setOpen((v) => !v));
  useHotkey({ key: "?", skipInInputs: true }, () => {
    if (!open) {
      setShowShortcuts(true);
      setOpen(true);
    }
  });

  useEffect(() => {
    if (!open) {
      setSearch("");
      setShowShortcuts(false);
    }
  }, [open]);

  const dispatch = (cmd: PaletteCommand) => {
    setOpen(false);
    switch (cmd.action.type) {
      case "set-range":
        setPreset(cmd.action.preset);
        return;
      case "navigate":
        push(cmd.action.page, cmd.action.filter);
        return;
      case "shortcuts":
        setShowShortcuts(true);
        setOpen(true);
        return;
    }
  };

  const dispatchAppItem = (app: string) => {
    setOpen(false);
    push("apps", { app });
  };

  const dispatchCategoryItem = (root: string) => {
    setOpen(false);
    push("categories", { category: root });
  };

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/60 pt-[12vh] backdrop-blur-sm"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) setOpen(false);
      }}
    >
      <Command
        label="Command palette"
        className="w-[min(640px,calc(100vw-32px))] overflow-hidden rounded-lg border bg-popover shadow-2xl"
        onKeyDown={(e) => {
          if (e.key === "Escape") setOpen(false);
        }}
      >
        {showShortcuts ? (
          <ShortcutsHelp onBack={() => setShowShortcuts(false)} />
        ) : (
          <>
            <div className="flex items-center gap-2.5 border-b px-3.5 py-2.5">
              <span className="text-muted-foreground">▍</span>
              <Command.Input
                value={search}
                onValueChange={setSearch}
                placeholder="Search Pulse…"
                autoFocus
                className="flex-1 border-0 bg-transparent text-sm text-foreground outline-none placeholder:text-muted-foreground"
              />
              <Badge variant="outline" className="font-mono tabular-nums">
                esc
              </Badge>
            </div>

            <Command.List className="max-h-[60vh] overflow-y-auto py-1.5">
              <Command.Empty className="py-6 text-center text-muted-foreground">
                No matching commands.
              </Command.Empty>

              {(["Range", "Navigate"] as const).map((g) => {
                const items = STATIC_COMMANDS.filter((c) => c.group === g);
                if (items.length === 0) return null;
                if (g === "Navigate" && search.trim() === "") return null;
                return (
                  <Command.Group key={g} heading={g} className={GROUP_HEADING_CLASS}>
                    {items.map((c) => (
                      <Command.Item
                        key={c.id}
                        value={`${g} ${c.label}`}
                        onSelect={() => dispatch(c)}
                        className={ITEM_CLASS}
                      >
                        <span className="inline-flex w-4 justify-center text-muted-foreground">
                          <HugeiconsIcon icon={c.icon} size={14} />
                        </span>
                        <span className="flex-1">{c.label}</span>
                        {c.hint && (
                          <Badge variant="outline" className="font-mono tabular-nums">
                            {c.hint}
                          </Badge>
                        )}
                      </Command.Item>
                    ))}
                  </Command.Group>
                );
              })}

              {(apps?.length ?? 0) > 0 && (
                <Command.Group heading="Apps" className={GROUP_HEADING_CLASS}>
                  {(apps ?? []).slice(0, DYNAMIC_TAKE).map((row) => (
                    <Command.Item
                      key={`app-${row.data.app}`}
                      value={`app ${row.data.app}`}
                      onSelect={() => dispatchAppItem(row.data.app)}
                      className={ITEM_CLASS}
                    >
                      <span className="inline-flex w-4 justify-center text-muted-foreground">
                        <HugeiconsIcon icon={DashboardSquare01Icon} size={14} />
                      </span>
                      <span className="flex-1 truncate">{row.data.app}</span>
                      <span className="font-mono tabular-nums text-muted-foreground">
                        {fmtDuration(row.duration)}
                      </span>
                    </Command.Item>
                  ))}
                </Command.Group>
              )}

              {(cats?.length ?? 0) > 0 && (
                <Command.Group heading="Categories" className={GROUP_HEADING_CLASS}>
                  {(cats ?? []).slice(0, DYNAMIC_TAKE).map((row) => {
                    const root = categoryRoot(row.name);
                    return (
                      <Command.Item
                        key={`cat-${row.name.join("/")}`}
                        value={`category ${row.name.join(" ")}`}
                        onSelect={() => dispatchCategoryItem(root)}
                        className={ITEM_CLASS}
                      >
                        <span className="inline-flex w-4 justify-center text-muted-foreground">
                          <HugeiconsIcon icon={PieChartIcon} size={14} />
                        </span>
                        <span className="flex-1 truncate">
                          {row.name.join(" / ")}
                        </span>
                        <span className="font-mono tabular-nums text-muted-foreground">
                          {fmtDuration(row.duration)}
                        </span>
                      </Command.Item>
                    );
                  })}
                </Command.Group>
              )}
            </Command.List>

            <footer className="flex items-center justify-between border-t bg-secondary/40 px-3.5 py-1.5 text-muted-foreground">
              <div className="flex items-center gap-3">
                <span>
                  <kbd className="font-mono tabular-nums">↑</kbd>{" "}
                  <kbd className="font-mono tabular-nums">↓</kbd> navigate
                </span>
                <span>
                  <kbd className="font-mono tabular-nums">↵</kbd> select
                </span>
                <span>
                  <kbd className="font-mono tabular-nums">esc</kbd> close
                </span>
              </div>
              <span>Pulse</span>
            </footer>
          </>
        )}
      </Command>
    </div>
  );
}

function ShortcutsHelp({ onBack }: { onBack: () => void }) {
  const rows: Array<[string, string]> = [
    ["⌘K · Ctrl+K", "Open command palette"],
    ["?", "Show this help"],
    ["Esc", "Close palette / dismiss detail view"],
    ["↑ ↓", "Navigate results"],
    ["↵", "Select highlighted result"],
  ];
  return (
    <div className="p-4">
      <div className="mb-3 flex items-center justify-between">
        <div className="font-semibold">Keyboard shortcuts</div>
        <Badge
          asChild
          variant="outline"
          className="cursor-pointer font-mono tabular-nums hover:bg-accent"
        >
          <button type="button" onClick={onBack}>back</button>
        </Badge>
      </div>
      <Separator className="mb-2" />
      <div className="flex flex-col">
        {rows.map(([k, v], i) => (
          <div
            key={k}
            className={
              "flex items-center gap-3 py-1.5 " +
              (i > 0 ? "border-t" : "")
            }
          >
            <span className="w-32 font-mono tabular-nums text-muted-foreground">
              {k}
            </span>
            <span>{v}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
