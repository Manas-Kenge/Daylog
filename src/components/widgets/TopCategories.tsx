/**
 * Top categories · pill rows. Bars are inset inside each pill so the row
 * functions as both the label/duration and the proportion indicator.
 */

import { ListBody, ListRow, WidgetCard } from "./Card";
import { useTopCategories } from "@/hooks/useAw";
import { fmtDuration } from "@/lib/format";
import { categoryColor } from "@/lib/category-colors";

export function TopCategories() {
  const { data } = useTopCategories();
  const total = (data ?? []).reduce((a, c) => a + c.duration, 0);

  return (
    <WidgetCard
      title="Top categories"
      description="Time per category"
      action={
        <span className="mono text-[10.5px] text-muted-foreground tracking-[0.13em] uppercase">
          {fmtDuration(total)} total
        </span>
      }
    >
      {!data || data.length === 0 ? (
        <div className="text-muted-foreground text-[12px] py-[16px] text-center">
          no categorized activity yet
        </div>
      ) : (
        <ListBody>
          {data.map((cat) => {
            const pct = total > 0 ? (cat.duration / total) * 100 : 0;
            const color = categoryColor(cat.name);
            const head = cat.name[0];
            const sub = cat.name[1] ? (
              <span className="text-muted-foreground font-normal text-[11px] ml-[5px]">
                / {cat.name[1]}
              </span>
            ) : null;
            return (
              <ListRow
                key={cat.name.join("/")}
                cols="9px_1fr_70px_60px"
              >
                <span
                  className="w-[8px] h-[8px] rounded-[2px]"
                  style={{ background: color }}
                />
                <span className="font-medium text-[12.5px] truncate">
                  {head}
                  {sub}
                </span>
                <span className="h-[3px] bg-background/50 rounded-[2px] overflow-hidden block">
                  <span
                    className="h-full block"
                    style={{ width: `${pct.toFixed(1)}%`, background: color }}
                  />
                </span>
                <span className="mono text-muted-foreground text-[11.5px] text-right">
                  {fmtDuration(cat.duration)}
                </span>
              </ListRow>
            );
          })}
        </ListBody>
      )}
    </WidgetCard>
  );
}
