import type { AppErrorCode } from "./errors";

/**
 * Channel label carried by `engine.toml` and persisted in the runtime registry. Free-form on the
 * backend; these are the labels Veil has shipped ("free" predates the ungoogled pivot).
 */
export type RuntimeChannel = "stable" | "free";

/**
 * Which launch-switch vocabulary the installed engine implements. Mirrors
 * `veil_core::FlagDialect`, declared by `crates/runtime/engine.toml`.
 *
 * `stock-chromium` is clean Chromium: it has no fingerprint switches, so the fingerprint surface
 * must show a degraded-capability state rather than pretending the settings take effect.
 */
export type FlagDialect = "stock-chromium" | "veil-patched";

/** A runtime that has been installed locally. */
export interface RuntimeVersion {
  version: string;
  channel: RuntimeChannel | string;
  executablePath: string;
  installedAt: string;
  sha256: string;
  sizeBytes: number | null;
}

/** A runtime discovered on GitHub Releases but not yet installed. */
export interface AvailableRuntime {
  version: string;
  assetName: string;
  downloadUrl: string;
  sha256: string;
  sizeBytes: number | null;
  publishedAt: string | null;
}

/** State of the currently-selected engine. */
export type RuntimeInstallState =
  { kind: "missing" } | { kind: "installed"; current: RuntimeVersion };

/** Which rule decided where engine runtimes are installed. Mirrors `EngineRootSource` in Rust. */
export type EngineRootSource =
  "envOverride" | "userSetting" | "nextToApp" | "legacyAdopted" | "devBuild" | "dataDir";

/**
 * Result of `get_runtime_install_root`. The reason travels with the path because the path alone
 * cannot be argued with: a fallback from an unwritable install directory is otherwise silent.
 */
export interface EngineRootInfo {
  path: string;
  source: EngineRootSource;
}

/**
 * Result of `get_data_location`. `pendingPath` is set when a move has been recorded but not yet
 * performed — it happens at the next startup, because the database is open while the app runs.
 */
export interface DataLocationInfo {
  path: string;
  isDefault: boolean;
  pendingPath: string | null;
}

/** Result of `list_installed_runtimes`. */
export interface InstalledRuntimes {
  currentVersion: string | null;
  installed: RuntimeVersion[];
  /**
   * Name of the **currently selected** engine, as recorded when it was installed. Falls back to
   * `engine.toml`'s name when nothing is selected or the entry predates capability recording.
   */
  engineName: string;
  /** Switch vocabulary the currently selected engine implements. */
  flagDialect: FlagDialect;
  /**
   * Whether `flagDialect` was read off that engine rather than defaulted.
   *
   * `false` means **unknown**, which is not the same claim as `stock-chromium` even though it is the
   * same behaviour: an engine installed before Veil recorded a dialect (or no engine at all) has
   * simply never been measured. Both degrade the fingerprint surface; only `true` justifies telling
   * the user their engine cannot spoof.
   */
  dialectRecorded: boolean;
}

/**
 * Placeholder used before `list_installed_runtimes` resolves, and when it fails. The dialect
 * degrades to `stock-chromium` on purpose: absent information means "assume no capability", so a
 * failed lookup can never make the UI claim protection the engine may not provide. `dialectRecorded`
 * is `false` for the same reason read the other way — nothing has been measured, so the UI must not
 * say it has.
 */
export function emptyInstalledRuntimes(): InstalledRuntimes {
  return {
    currentVersion: null,
    installed: [],
    engineName: "",
    flagDialect: "stock-chromium",
    dialectRecorded: false,
  };
}

/**
 * Live install progress. Emitted by the backend over a Tauri `Channel` while
 * `install_runtime_version` is running.
 */
export type RuntimeProgress =
  | {
      kind: "downloading";
      downloaded: number;
      total: number | null;
      bytesPerSec: number;
    }
  | { kind: "verifying" }
  | { kind: "extracting"; entriesDone: number; entriesTotal: number }
  | { kind: "finalizing" }
  | { kind: "completed"; version: string }
  | { kind: "cancelled"; version: string }
  | { kind: "failed"; version: string; code: AppErrorCode };

/** Terminal progress events end the install state machine. */
export function isRuntimeProgressTerminal(progress: RuntimeProgress): boolean {
  return (
    progress.kind === "completed" || progress.kind === "cancelled" || progress.kind === "failed"
  );
}
