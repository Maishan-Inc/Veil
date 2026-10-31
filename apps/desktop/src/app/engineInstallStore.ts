import type { AppErrorCode, AvailableRuntime, RuntimeProgress } from "@veil/shared";
import { create } from "zustand";
import {
  AppError,
  cancelRuntimeInstall,
  installRuntimeFromLocalZip,
  installRuntimeVersion,
  setCurrentRuntime,
} from "../api/client";
import { useToastStore } from "./toastStore";

export type RuntimeInstallTarget =
  | { source: "remote"; candidate: AvailableRuntime }
  | { source: "localZip"; candidate: AvailableRuntime; localZipPath: string };

export interface EngineInstallJob {
  target: RuntimeInstallTarget;
  progress: RuntimeProgress | null;
  /** Set when the pipeline failed. A cancel clears the job instead of landing here. */
  errorCode: AppErrorCode | null;
}

export interface EngineInstallStoreState {
  /** The one install in flight, or the one that failed and is waiting to be retried. */
  job: EngineInstallJob | null;
  /** Bumped after a successful install so any mounted screen can re-fetch the registry. */
  completedCount: number;

  start: (target: RuntimeInstallTarget) => Promise<void>;
  cancel: () => Promise<void>;
  /** Drop a failed job from the dock without retrying. */
  dismiss: () => void;
}

/** The install the dock is allowed to talk about, if any. */
export const selectInstallJob = (state: EngineInstallStoreState): EngineInstallJob | null =>
  state.job;

/**
 * Engine install, hoisted out of `BrowserPage`.
 *
 * It lives here for one reason: a 200 MB download outlives the dialog that started it and the
 * page that owns the dialog. While this was component state, closing the modal or navigating to
 * another section unmounted the only listener for the progress Channel, so the install kept
 * running with nothing rendering it. The store keeps the job addressable from the app shell.
 *
 * Business logic still lives in Rust — this only sequences the two commands an install needs
 * (`install_*` then `set_current_runtime`) and decides which toast the outcome deserves.
 * One job at a time, matching the backend, whose cancel tokens are keyed per version but whose
 * registry write is not concurrent-safe against a second install.
 */
export const useEngineInstallStore = create<EngineInstallStoreState>((set, get) => ({
  job: null,
  completedCount: 0,

  start: async (target) => {
    const version = target.candidate.version;
    set({ job: { target, progress: null, errorCode: null } });

    const handleProgress = (progress: RuntimeProgress): void => {
      set((state) => {
        // A late event from a superseded job must not resurrect it.
        if (!state.job || state.job.target.candidate.version !== version) return {};
        const errorCode = progress.kind === "failed" ? progress.code : state.job.errorCode;
        return { job: { ...state.job, progress, errorCode } };
      });
    };

    try {
      await (target.source === "localZip"
        ? installRuntimeFromLocalZip(target.candidate, target.localZipPath, handleProgress)
        : installRuntimeVersion(target.candidate, handleProgress));
      // A freshly installed engine is the one the user meant to use; the old flow did this too.
      await setCurrentRuntime(version);
      set((state) => ({ job: null, completedCount: state.completedCount + 1 }));
      useToastStore.getState().push({
        tone: "success",
        titleKey: "browser.toast.installed.title",
        bodyKey: "browser.toast.installed.body",
        params: { version },
        ttlMs: 7000,
      });
    } catch (err) {
      const code: AppErrorCode = err instanceof AppError ? err.code : "app.unknown";
      // Cancelling is a user decision, not a failure to report back to them.
      if (code === "runtime.installCancelled") {
        set((state) => (state.job?.target.candidate.version === version ? { job: null } : {}));
        return;
      }
      set((state) =>
        state.job && state.job.target.candidate.version === version
          ? { job: { ...state.job, errorCode: code } }
          : {},
      );
    }
  },

  cancel: async () => {
    const job = get().job;
    if (!job) return;
    try {
      await cancelRuntimeInstall(job.target.candidate.version);
    } catch {
      // Best-effort: the install promise still settles and clears the job.
    }
  },

  dismiss: () => set({ job: null }),
}));
