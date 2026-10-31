import { create } from "zustand";

export type ToastTone = "info" | "success" | "warning" | "error";

/**
 * A queued toast. Holds **i18n keys, not resolved strings**: the locale can change while a
 * toast is on screen, and `@veil/i18n`'s `t` is a hook the store cannot call. The host resolves
 * them at render time.
 */
export interface ToastMessage {
  id: string;
  tone: ToastTone;
  titleKey: string;
  bodyKey?: string | undefined;
  /** Interpolation params, applied to both keys. */
  params?: Record<string, string | number> | undefined;
  /** Milliseconds before auto-dismiss. `0` keeps it until the user closes it. */
  ttlMs: number;
}

export interface ToastStoreState {
  toasts: ToastMessage[];
  /** Enqueue a toast and return its id. */
  push: (toast: Omit<ToastMessage, "id">) => string;
  dismiss: (id: string) => void;
  clear: () => void;
}

/** Oldest toasts drop out first once this many are visible. */
const MAX_VISIBLE = 4;

let sequence = 0;

/**
 * App-level toast queue.
 *
 * Deliberately separate from `useAppStore`: that store is navigation/theme/selection state read
 * by nearly every screen, and a queue that mutates on every background event would re-render
 * all of them. Nothing here talks to Tauri — callers decide *when* a toast is warranted.
 */
export const useToastStore = create<ToastStoreState>((set) => ({
  toasts: [],

  push: (toast) => {
    sequence += 1;
    const id = `toast-${sequence}`;
    set((state) => ({ toasts: [...state.toasts, { ...toast, id }].slice(-MAX_VISIBLE) }));
    return id;
  },

  dismiss: (id) => set((state) => ({ toasts: state.toasts.filter((entry) => entry.id !== id) })),

  clear: () => set({ toasts: [] }),
}));
