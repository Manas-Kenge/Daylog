import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { WizardShell } from "@/components/wizard/WizardShell";
import { tracking, type Detection, type ExtensionStatus } from "@/lib/tracking";

interface WizardProps {
  onComplete: () => Promise<void>;
}

type State =
  | { kind: "detecting" }
  | { kind: "existing-aw"; info: { hostname: string; version: string } }
  | { kind: "ready-to-install" }
  | { kind: "installing" }
  | { kind: "install-error"; error: string }
  | { kind: "gnome-prompt"; status: ExtensionStatus }
  | { kind: "gnome-installing" }
  | { kind: "gnome-error"; error: string }
  | { kind: "gnome-degraded" }
  | { kind: "needs-relogin" }
  | { kind: "ready-to-finish" };

export function Wizard({ onComplete }: WizardProps) {
  const [state, setState] = useState<State>({ kind: "detecting" });

  useEffect(() => {
    let cancelled = false;
    tracking.detect().then((d: Detection) => {
      if (cancelled) return;
      if (d.kind === "existing") {
        setState({ kind: "existing-aw", info: { hostname: d.hostname, version: d.version } });
      } else {
        setState({ kind: "ready-to-install" });
      }
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const startInstall = async () => {
    setState({ kind: "installing" });
    try {
      await tracking.installSupervisor();
      const ext = await tracking.gnomeExtensionStatus();
      branchOnExtension(ext);
    } catch (e: unknown) {
      setState({ kind: "install-error", error: String(e) });
    }
  };

  const branchOnExtension = (ext: ExtensionStatus) => {
    if (!ext.applicable) {
      setState({ kind: "ready-to-finish" });
    } else if (!ext.available) {
      setState({ kind: "gnome-degraded" });
    } else if (ext.installed && ext.enabled) {
      // Already in place from a prior install or another tool. Skip.
      setState({ kind: "ready-to-finish" });
    } else {
      setState({ kind: "gnome-prompt", status: ext });
    }
  };

  const installGnomeExtension = async () => {
    setState({ kind: "gnome-installing" });
    try {
      const ext = await tracking.setupGnomeExtension();
      if (ext.needs_relogin) {
        setState({ kind: "needs-relogin" });
      } else {
        setState({ kind: "ready-to-finish" });
      }
    } catch (e: unknown) {
      setState({ kind: "gnome-error", error: String(e) });
    }
  };

  const finish = async () => {
    await onComplete();
  };

  return renderStep(state, {
    startInstall,
    retryInstall: startInstall,
    installGnomeExtension,
    skipGnome: () => setState({ kind: "ready-to-finish" }),
    finish,
  });
}

interface Handlers {
  startInstall: () => void;
  retryInstall: () => void;
  installGnomeExtension: () => void;
  skipGnome: () => void;
  finish: () => void;
}

function renderStep(state: State, h: Handlers) {
  switch (state.kind) {
    case "detecting":
      return (
        <WizardShell
          title="Welcome to Daylog"
          description="Looking for ActivityWatch on this machine…"
          body={<Spinner label="Probing localhost:5600" />}
        />
      );

    case "existing-aw":
      return (
        <WizardShell
          title="Using your existing ActivityWatch install"
          description={
            <>
              Daylog detected ActivityWatch <code>{state.info.version}</code> already running on{" "}
              <code>{state.info.hostname}</code>. We'll use that instead of installing a second copy.
            </>
          }
          body={
            <p className="text-sm text-muted-foreground">
              Tracking continues to be managed by your existing setup. Daylog just renders the data.
            </p>
          }
          footer={<Button onClick={h.finish}>Continue to dashboard</Button>}
        />
      );

    case "ready-to-install":
      return (
        <WizardShell
          title="Set up tracking"
          description="Daylog will install a small, always-on tracker that runs whenever you're logged in. Like Screen Time on macOS."
          body={
            <ul className="space-y-1 text-sm text-muted-foreground">
              <li>• Runs as a user-level service (no root needed).</li>
              <li>• Stops automatically when you log out.</li>
              <li>• Stores everything locally — never leaves your machine.</li>
            </ul>
          }
          footer={<Button onClick={h.startInstall}>Set up tracking</Button>}
        />
      );

    case "installing":
      return (
        <WizardShell
          title="Setting up tracker"
          description="This takes about 10 seconds."
          body={<Spinner label="Installing service and waiting for first event…" />}
        />
      );

    case "install-error":
      return (
        <WizardShell
          title="Tracker setup failed"
          description="Something went wrong installing the tracker. The error is below; you can copy it or retry."
          body={
            <p className="text-sm text-muted-foreground">
              For systemd, you can also inspect logs with{" "}
              <code>journalctl --user -u daylog-aw-server</code>.
            </p>
          }
          error={state.error}
          footer={<Button onClick={h.retryInstall}>Try again</Button>}
        />
      );

    case "gnome-prompt":
      return (
        <WizardShell
          title="One more step for GNOME on Wayland"
          description="GNOME on Wayland doesn't expose window titles to background processes. Daylog can install a small extension that fixes this — locally, no root."
          body={
            <p className="text-sm text-muted-foreground">
              The extension is bundled inside Daylog; installing it doesn't reach the network.
            </p>
          }
          footer={
            <>
              <Button variant="ghost" onClick={h.skipGnome}>
                Skip
              </Button>
              <Button onClick={h.installGnomeExtension}>Install extension</Button>
            </>
          }
        />
      );

    case "gnome-installing":
      return (
        <WizardShell
          title="Installing GNOME extension"
          body={<Spinner label="Running gnome-extensions install + enable…" />}
        />
      );

    case "gnome-error":
      return (
        <WizardShell
          title="Extension setup failed"
          description="Daylog couldn't install the GNOME extension. You can skip this step — tracking will work, but window titles may be missing on GNOME-Wayland sessions."
          error={state.error}
          body={null}
          footer={<Button onClick={h.skipGnome}>Continue anyway</Button>}
        />
      );

    case "gnome-degraded":
      return (
        <WizardShell
          title="GNOME extensions support is disabled"
          description="Daylog couldn't find the gnome-extensions command. Your GNOME-Wayland session may not show window titles. Tracking still works for time totals."
          body={
            <p className="text-sm text-muted-foreground">
              You can revisit this in Settings → Tracking once GNOME extensions are available.
            </p>
          }
          footer={<Button onClick={h.finish}>Continue to dashboard</Button>}
        />
      );

    case "needs-relogin":
      return (
        <WizardShell
          title="Almost there — please log out and back in"
          description="GNOME-Wayland can't reload extensions live. Once you log back in, Daylog will start receiving window-title data automatically."
          body={
            <p className="text-sm text-muted-foreground">
              You can keep using Daylog now; window titles will start appearing after the next login.
            </p>
          }
          footer={<Button onClick={h.finish}>Open dashboard</Button>}
        />
      );

    case "ready-to-finish":
      return (
        <WizardShell
          title="All set"
          description="Daylog is tracking your activity. Open the dashboard whenever you want to see what you've done today."
          body={null}
          footer={<Button onClick={h.finish}>Open dashboard</Button>}
        />
      );
  }
}

function Spinner({ label }: { label: string }) {
  return (
    <div className="flex items-center gap-3 py-4 text-sm text-muted-foreground">
      <span
        aria-hidden
        className="block h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent"
      />
      <span>{label}</span>
    </div>
  );
}
