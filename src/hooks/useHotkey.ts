/**
 * Lightweight global hotkey binding. Single window-level listener, cleaned
 * up on unmount. Modifier-aware: pass `meta: true` for ⌘ and `ctrl: true`
 * for Ctrl. Both can be true to bind both (e.g., palette open).
 */

import { useEffect } from "react";

export interface HotkeyOptions {
  key: string;
  meta?: boolean;
  ctrl?: boolean;
  /** When true, prevents default browser behavior. Default: true. */
  preventDefault?: boolean;
  /** Skip when focused element is an input/textarea/contenteditable. */
  skipInInputs?: boolean;
}

export function useHotkey(
  options: HotkeyOptions,
  handler: (e: KeyboardEvent) => void,
) {
  const {
    key,
    meta = false,
    ctrl = false,
    preventDefault = true,
    skipInInputs = false,
  } = options;

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key.toLowerCase() !== key.toLowerCase()) return;

      const matchesMeta = meta && e.metaKey;
      const matchesCtrl = ctrl && e.ctrlKey;
      const matchesNoMod = !meta && !ctrl && !e.metaKey && !e.ctrlKey && !e.altKey;
      if (!(matchesMeta || matchesCtrl || matchesNoMod)) return;

      if (skipInInputs) {
        const t = e.target as HTMLElement | null;
        const tag = t?.tagName;
        if (
          tag === "INPUT" ||
          tag === "TEXTAREA" ||
          (t && t.isContentEditable)
        ) {
          return;
        }
      }

      if (preventDefault) e.preventDefault();
      handler(e);
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [key, meta, ctrl, preventDefault, skipInInputs, handler]);
}
