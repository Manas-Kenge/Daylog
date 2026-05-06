/**
 * Map category root (the first level of `name: string[]`) to a chart color
 * from the design tokens. Falls through to chart-1 for unknown roots so
 * categorization stays consistent across widgets.
 */

const ROOT_TO_COLOR: Record<string, string> = {
  Work: "var(--chart-1)",        // purple
  Programming: "var(--chart-1)", // alias when Work/Programming collapses
  Comms: "var(--chart-2)",       // pink
  Media: "var(--chart-3)",       // amber
  Browsing: "var(--chart-4)",    // blue
  Documents: "var(--chart-5)",   // green
  Uncategorized: "var(--accent)",
};

export function categoryColor(name: string[]): string {
  if (name.length === 0) return ROOT_TO_COLOR.Uncategorized;
  // Try the deepest known segment first, then walk up.
  for (let i = name.length - 1; i >= 0; i--) {
    const seg = name[i];
    if (seg in ROOT_TO_COLOR) return ROOT_TO_COLOR[seg];
  }
  return "var(--chart-1)";
}

export function categoryLabel(name: string[]): string {
  if (name.length === 0) return "Uncategorized";
  return name.join(" / ");
}

export function categoryRoot(name: string[]): string {
  return name[0] ?? "Uncategorized";
}
