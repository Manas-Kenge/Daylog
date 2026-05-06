import { createContext, useContext, useMemo, useState, type ReactNode } from "react";
import type { TimeRange } from "@/lib/aw-types";
import { LastNDays, Today, Yesterday } from "@/lib/aw-types";

export type RangePreset = "today" | "yesterday" | "7d" | "30d" | "custom";

export const PRESETS: Record<Exclude<RangePreset, "custom">, TimeRange> = {
  today: Today,
  yesterday: Yesterday,
  "7d": LastNDays(7),
  "30d": LastNDays(30),
};

interface RangeContextValue {
  preset: RangePreset;
  range: TimeRange;
  setPreset: (preset: RangePreset) => void;
  setRange: (range: TimeRange) => void;
}

const RangeContext = createContext<RangeContextValue | null>(null);

export function RangeProvider({ children }: { children: ReactNode }) {
  const [preset, setPresetState] = useState<RangePreset>("today");
  const [customRange, setCustomRange] = useState<TimeRange | null>(null);

  const value = useMemo<RangeContextValue>(() => {
    const range: TimeRange =
      preset === "custom" && customRange ? customRange : PRESETS[preset as keyof typeof PRESETS] ?? Today;

    return {
      preset,
      range,
      setPreset: (next) => {
        setPresetState(next);
        if (next !== "custom") setCustomRange(null);
      },
      setRange: (next) => {
        setCustomRange(next);
        setPresetState("custom");
      },
    };
  }, [preset, customRange]);

  return <RangeContext.Provider value={value}>{children}</RangeContext.Provider>;
}

export function useRange(): RangeContextValue {
  const ctx = useContext(RangeContext);
  if (!ctx) throw new Error("useRange must be used inside <RangeProvider>");
  return ctx;
}
