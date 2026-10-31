export type ThemePreference = "light" | "dark" | "auto";
export type ResolvedTheme = "light" | "dark";

const STORAGE_KEY = "veil.theme";

/** Resolve a preference (including "auto") to a concrete theme. */
export function resolveTheme(pref: ThemePreference): ResolvedTheme {
  if (pref === "auto") {
    const prefersDark =
      typeof window !== "undefined" &&
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-color-scheme: dark)").matches;
    return prefersDark ? "dark" : "light";
  }
  return pref;
}

/** Apply a preference to the document root and return the resolved theme. */
export function applyTheme(pref: ThemePreference): ResolvedTheme {
  const resolved = resolveTheme(pref);
  if (typeof document !== "undefined") {
    const root = document.documentElement;
    const previous = root.getAttribute("data-theme");
    // Only crossfade when switching between two concrete themes — never on first paint
    // (there is nothing to fade from) and never when the value is unchanged.
    if (previous && previous !== resolved) {
      enableThemeTransition(root);
    }
    root.setAttribute("data-theme", resolved);
  }
  return resolved;
}

let themeTransitionTimer: ReturnType<typeof setTimeout> | undefined;

/**
 * Briefly enables a color crossfade on the root, then removes it so the transition never
 * lingers on regular hover/focus interactions. Matches --veil-duration-slow (240ms).
 */
function enableThemeTransition(root: HTMLElement): void {
  root.classList.add("veil-theme-switching");
  if (themeTransitionTimer !== undefined) clearTimeout(themeTransitionTimer);
  themeTransitionTimer = setTimeout(() => {
    root.classList.remove("veil-theme-switching");
    themeTransitionTimer = undefined;
  }, 260);
}

export function loadThemePreference(): ThemePreference {
  if (typeof localStorage === "undefined") return "dark";
  const value = localStorage.getItem(STORAGE_KEY);
  return value === "light" || value === "dark" || value === "auto" ? value : "dark";
}

export function saveThemePreference(pref: ThemePreference): void {
  if (typeof localStorage !== "undefined") {
    localStorage.setItem(STORAGE_KEY, pref);
  }
}

/** Subscribe to OS theme changes (only relevant while preference is "auto"). */
export function watchSystemTheme(callback: (resolved: ResolvedTheme) => void): () => void {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return () => {};
  }
  const query = window.matchMedia("(prefers-color-scheme: dark)");
  const handler = (): void => callback(query.matches ? "dark" : "light");
  query.addEventListener("change", handler);
  return () => query.removeEventListener("change", handler);
}
