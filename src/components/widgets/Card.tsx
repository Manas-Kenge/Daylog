/**
 * Pulse widget card. Mirrors databuddy's `DataTable` shape:
 *   <Card>
 *     <Toolbar title description action />
 *     [optional Tabs]
 *     <Body>{children}</Body>
 *   </Card>
 *
 * Outer wrapper is the bordered shell; the body slot is rendered as-is so
 * widgets can opt in or out of the recessed list pattern (`<ListBody>`
 * below) per their content.
 */

import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

interface CardProps {
  title: string;
  description?: ReactNode;
  action?: ReactNode;
  bodyClassName?: string;
  className?: string;
  children: ReactNode;  
}

export function WidgetCard({
  title,
  description,
  action,
  bodyClassName,
  className,
  children,
}: CardProps) {
  return (
    <section
      className={cn(
        "bg-card border border-border rounded-[var(--radius-lg)] overflow-hidden flex flex-col min-w-0",
        className,
      )}
    >
      <header className="px-[14px] py-[10px] flex items-start justify-between gap-[12px] border-b border-border">
        <div className="min-w-0">
          <div className="font-semibold text-[13px] text-foreground tracking-tight">
            {title}
          </div>
          {description && (
            <div className="text-[11.5px] text-muted-foreground mt-[2px]">
              {description}
            </div>
          )}
        </div>
        {action && <div className="shrink-0">{action}</div>}
      </header>
      <div className={cn("p-[10px]", bodyClassName)}>{children}</div>
    </section>
  );
}

/**
 * Recessed list body — `bg-background p-1 rounded-md`, with pill rows inside.
 * Children should be `<ListRow>` siblings.
 */
export function ListBody({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "bg-background rounded-[var(--radius-md)] p-[4px] flex flex-col gap-[2px]",
        className,
      )}
    >
      {children}
    </div>
  );
}

/**
 * Single pill row inside a `<ListBody>`. Use the `cols` prop with a Tailwind
 * grid template (e.g. `"9px_1fr_auto_auto"`).
 */
export function ListRow({
  cols = "9px_1fr_auto_auto",
  className,
  title: rowTitle,
  children,
}: {
  cols?: string;
  className?: string;
  title?: string;
  children: ReactNode;
}) {
  return (
    <div
      title={rowTitle}
      className={cn(
        "bg-muted/30 hover:bg-muted/60 rounded-[var(--radius-sm)] transition-colors px-[10px] py-[6px] grid items-center gap-[10px]",
        className,
      )}
      style={{ gridTemplateColumns: cols.replace(/_/g, " ") }}
    >
      {children}
    </div>
  );
}
