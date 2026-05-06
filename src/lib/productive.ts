/**
 * "Time in Work" classification.
 *
 * For v0.1, a single category root counts as Work: "Work". Anything under
 * Work/* counts; everything else does not, because we can't tell
 * e.g. Slack-for-work from Discord-for-play without per-rule classification.
 *
 * Renamed from `productive` post-CEO-review (PLAN.md §1.0). Pulse is
 * observational; "productive" implies the rest of the day was unproductive,
 * which we don't claim. The v0.2 settings UI will let users edit `WORK_ROOTS`.
 */

export const WORK_ROOTS: readonly string[] = ["Work"];

export function isWork(category: readonly string[]): boolean {
  return category.length > 0 && WORK_ROOTS.includes(category[0]);
}
