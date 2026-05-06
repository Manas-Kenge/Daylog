import type { ReactNode } from "react";
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { cn } from "@/lib/utils";

interface WidgetCardProps {
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
}: WidgetCardProps) {
  return (
    <Card size="sm" className={cn("h-full", className)}>
      <CardHeader className="border-b">
        <CardTitle>{title}</CardTitle>
        {description ? <CardDescription>{description}</CardDescription> : null}
        {action ? <CardAction>{action}</CardAction> : null}
      </CardHeader>
      <CardContent className={cn("flex-1 min-h-0", bodyClassName)}>
        {children}
      </CardContent>
    </Card>
  );
}

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
        "flex flex-col gap-0.5 rounded-md bg-background p-1",
        className,
      )}
    >
      {children}
    </div>
  );
}

export function ListRow({
  cols = "9px_1fr_auto_auto",
  className,
  title,
  children,
  onClick,
}: {
  cols?: string;
  className?: string;
  title?: string;
  children: ReactNode;
  /** When provided, the row becomes a button-shaped focusable element so
   *  click-to-filter wiring is keyboard-accessible (PLAN.md §1.0 wedge). */
  onClick?: () => void;
}) {
  const interactive = onClick != null;
  return (
    <div
      title={title}
      role={interactive ? "button" : undefined}
      tabIndex={interactive ? 0 : undefined}
      onClick={onClick}
      onKeyDown={
        interactive
          ? (e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                onClick!();
              }
            }
          : undefined
      }
      className={cn(
        "grid items-center gap-2.5 rounded-sm bg-muted/30 px-2.5 py-1.5 transition-colors hover:bg-muted/60",
        interactive &&
          "focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring",
        className,
      )}
      style={{ gridTemplateColumns: cols.replace(/_/g, " ") }}
    >
      {children}
    </div>
  );
}
