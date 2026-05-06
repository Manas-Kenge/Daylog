import type { ReactNode } from "react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { cn } from "@/lib/utils";

interface WizardShellProps {
  title: string;
  description?: ReactNode;
  body: ReactNode;
  footer?: ReactNode;
  error?: string | null;
  className?: string;
}

/**
 * Centered, single-card frame used by every step. Keeps padding, max-width,
 * and the error surface consistent so step components stay short.
 */
export function WizardShell({
  title,
  description,
  body,
  footer,
  error,
  className,
}: WizardShellProps) {
  return (
    <div className="flex h-screen w-screen items-center justify-center bg-background px-6">
      <Card className={cn("w-full max-w-xl", className)}>
        <CardHeader>
          <CardTitle>{title}</CardTitle>
          {description ? <CardDescription>{description}</CardDescription> : null}
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <div>{body}</div>
          {error ? <ErrorPanel message={error} /> : null}
          {footer ? <div className="flex justify-end gap-2 pt-2">{footer}</div> : null}
        </CardContent>
      </Card>
    </div>
  );
}

function ErrorPanel({ message }: { message: string }) {
  return (
    <div className="flex flex-col gap-2 rounded-md border border-destructive/50 bg-destructive/10 p-3">
      <div className="text-sm font-medium text-destructive">Something went wrong</div>
      <pre className="overflow-x-auto whitespace-pre-wrap break-words text-xs text-muted-foreground">
        {message}
      </pre>
      <button
        type="button"
        className="self-start text-xs text-muted-foreground underline-offset-2 hover:underline"
        onClick={() => navigator.clipboard.writeText(message)}
      >
        Copy
      </button>
    </div>
  );
}
