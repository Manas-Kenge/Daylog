/**
 * Static command registry. Dynamic commands (top apps, top categories) are
 * composed inside CommandPalette.tsx because they depend on TanStack Query
 * hooks that can't run outside the component tree.
 */

import {
  Activity03Icon,
  Calendar01Icon,
  CalendarMinus02Icon,
  Clock01Icon,
  DashboardSquare01Icon,
  GlobalIcon,
  Note01Icon,
  PieChartIcon,
  Calendar03Icon,
} from "@hugeicons/core-free-icons";
import type { IconSvgElement } from "@hugeicons/react";
import type { PageFilter, PageId } from "@/context/PageContext";
import type { RangePreset } from "@/context/RangeContext";

export type CommandAction =
  | { type: "set-range"; preset: RangePreset }
  | { type: "navigate"; page: PageId; filter?: PageFilter }
  | { type: "shortcuts" };

export interface PaletteCommand {
  /** Stable id; cmdk uses this for keyboard select. */
  id: string;
  /** Visible label in the result list. */
  label: string;
  /** Optional secondary hint shown right-aligned. */
  hint?: string;
  /** Hugeicons icon shown as the row glyph. */
  icon: IconSvgElement;
  /** Action to dispatch when selected. */
  action: CommandAction;
  /** Group title under which this command renders. */
  group: "Range" | "Navigate" | "Settings" | "Help";
}

export const RANGE_COMMANDS: PaletteCommand[] = [
  { id: "range-today",     label: "Today",      icon: Calendar01Icon,    group: "Range", action: { type: "set-range", preset: "today" } },
  { id: "range-yesterday", label: "Yesterday",  icon: CalendarMinus02Icon, group: "Range", action: { type: "set-range", preset: "yesterday" } },
  { id: "range-7d",        label: "This week",  icon: Calendar03Icon,    group: "Range", action: { type: "set-range", preset: "7d" } },
  { id: "range-30d",       label: "This month", icon: Calendar01Icon,    group: "Range", action: { type: "set-range", preset: "30d" } },
];

/**
 * Detail-page navigation commands. These are kept in the registry so that
 * typing "apps", "hourly", etc. in the palette finds them — but the palette
 * hides this group on an empty query so the open-state stays focused on
 * range switching + dynamic search (PLAN §1: "important stuff only").
 */
export const NAV_COMMANDS: PaletteCommand[] = [
  { id: "nav-apps",       label: "Apps",            icon: DashboardSquare01Icon, group: "Navigate", action: { type: "navigate", page: "apps" } },
  { id: "nav-categories", label: "Categories",      icon: PieChartIcon,          group: "Navigate", action: { type: "navigate", page: "categories" } },
  { id: "nav-hourly",     label: "Hourly patterns", icon: Clock01Icon,           group: "Navigate", action: { type: "navigate", page: "hourly" } },
  { id: "nav-activity",   label: "Activity log",    icon: Note01Icon,            group: "Navigate", action: { type: "navigate", page: "activity" } },
  { id: "nav-web",        label: "Web",             icon: GlobalIcon,            group: "Navigate", action: { type: "navigate", page: "web" } },
];

export { Activity03Icon };

export const STATIC_COMMANDS: PaletteCommand[] = [
  ...RANGE_COMMANDS,
  ...NAV_COMMANDS,
];
