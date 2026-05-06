import { invoke } from "@tauri-apps/api/core";

export type Detection =
  | { kind: "existing"; hostname: string; version: string }
  | { kind: "none" };

export type Supervisor = "systemd" | "xdg-autostart" | "external";
export type UnitState = "active" | "inactive" | "failed" | "unknown";

export interface TrackerStatus {
  supervisor: Supervisor;
  server: UnitState;
  watcher: UnitState;
}

export type BinDirSource = "app-image-extracted" | "system-package" | "development";

export interface BinDir {
  path: string;
  source: BinDirSource;
  stamped_version: string | null;
}

export interface ExtensionStatus {
  applicable: boolean;
  available: boolean;
  installed: boolean;
  enabled: boolean;
  needs_relogin: boolean;
}

export const tracking = {
  detect: () => invoke<Detection>("tracking_detect"),
  detectSupervisor: () => invoke<Supervisor>("tracking_detect_supervisor"),
  resolveBinDir: () => invoke<BinDir>("tracking_resolve_bin_dir"),
  placeBinaries: () => invoke<BinDir>("tracking_place_binaries"),
  installSupervisor: () => invoke<TrackerStatus>("tracking_install_supervisor"),
  status: () => invoke<TrackerStatus>("tracking_status"),
  pause: () => invoke<void>("tracking_pause"),
  resume: () => invoke<void>("tracking_resume"),
  stop: () => invoke<void>("tracking_stop"),
  gnomeExtensionStatus: () => invoke<ExtensionStatus>("tracking_gnome_extension_status"),
  setupGnomeExtension: () => invoke<ExtensionStatus>("tracking_setup_gnome_extension"),
};

export const wizard = {
  isComplete: () => invoke<boolean>("wizard_complete_get"),
  setComplete: (complete: boolean) =>
    invoke<void>("wizard_complete_set", { complete }),
};
