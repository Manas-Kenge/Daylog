/**
 * Productive-time classification.
 *
 * For v0.1, we treat a single category root as "productive": "Work".
 * Anything categorized under Work/* counts; everything else (Comms,
 * Browsing, Media, Uncategorized) does NOT count, because we can't
 * tell e.g. Slack-for-work from Discord-for-play, or research-browsing
 * from Twitter-browsing, without per-rule classification.
 *
 * Future: replace with a user-editable allowlist persisted via
 * tauri-plugin-store, and add a `productive: boolean` field on
 * category rules so e.g. work-Slack and personal-Discord can diverge.
 */

export const PRODUCTIVE_ROOTS: readonly string[] = ["Work"];

export function isProductive(category: readonly string[]): boolean {
  return category.length > 0 && PRODUCTIVE_ROOTS.includes(category[0]);
}
