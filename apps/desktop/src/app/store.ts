import { locales, type Locale } from "@veil/i18n";
import {
  applyTheme,
  loadThemePreference,
  saveThemePreference,
  type ThemePreference,
} from "@veil/ui";
import { create } from "zustand";

export type Section =
  "profiles" | "kernel" | "proxy" | "ai" | "extensions" | "api" | "settings" | "logs";

/**
 * Pure view state. Holds no business logic — services live in Rust, the API client is the
 * only bridge. The store tracks navigation, theme, locale, search, and row selection.
 */
export interface AppStoreState {
  section: Section;
  sidebarCollapsed: boolean;
  themePreference: ThemePreference;
  locale: Locale;
  search: string;
  selectedProfileIds: Set<string>;
  /** Incremented when profile data has been mutated; consumers re-fetch on change. */
  profilesVersion: number;

  setSection: (section: Section) => void;
  toggleSidebar: () => void;
  setThemePreference: (pref: ThemePreference) => void;
  setLocale: (locale: Locale) => void;
  setSearch: (value: string) => void;
  toggleSelection: (id: string) => void;
  replaceSelection: (ids: Iterable<string>) => void;
  clearSelection: () => void;
  bumpProfiles: () => void;
}

const STORAGE_LOCALE_KEY = "veil.locale";

const localeSet = new Set<string>(locales);

function loadLocale(): Locale {
  if (typeof localStorage !== "undefined") {
    const stored = localStorage.getItem(STORAGE_LOCALE_KEY);
    if (stored && localeSet.has(stored)) return stored as Locale;
  }
  const languages =
    typeof navigator === "undefined" ? [] : [navigator.language, ...(navigator.languages ?? [])];
  for (const language of languages) {
    const normalized = normalizeLocale(language);
    if (normalized) return normalized;
  }
  return "en-US";
}

function persistLocale(locale: Locale): void {
  if (typeof localStorage !== "undefined") {
    localStorage.setItem(STORAGE_LOCALE_KEY, locale);
  }
}

function normalizeLocale(language: string | undefined): Locale | null {
  const value = (language ?? "").trim();
  if (!value) return null;
  if (localeSet.has(value)) return value as Locale;
  const lower = value.toLowerCase();
  if (
    lower.startsWith("zh-hant") ||
    lower.startsWith("zh-tw") ||
    lower.startsWith("zh-hk") ||
    lower.startsWith("zh-mo")
  ) {
    return "zh-TW";
  }
  if (lower.startsWith("zh")) return "zh-CN";
  if (lower.startsWith("ja")) return "ja-JP";
  if (lower.startsWith("ru")) return "ru-RU";
  if (lower.startsWith("en")) return "en-US";
  return null;
}

export const useAppStore = create<AppStoreState>((set) => ({
  section: "profiles",
  sidebarCollapsed: false,
  themePreference: loadThemePreference(),
  locale: loadLocale(),
  search: "",
  selectedProfileIds: new Set<string>(),
  profilesVersion: 0,

  setSection: (section) => set({ section }),
  toggleSidebar: () => set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed })),
  setThemePreference: (pref) => {
    saveThemePreference(pref);
    applyTheme(pref);
    set({ themePreference: pref });
  },
  setLocale: (locale) => {
    persistLocale(locale);
    set({ locale });
  },
  setSearch: (search) => set({ search }),
  toggleSelection: (id) =>
    set((state) => {
      const next = new Set(state.selectedProfileIds);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return { selectedProfileIds: next };
    }),
  replaceSelection: (ids) => set({ selectedProfileIds: new Set(ids) }),
  clearSelection: () => set({ selectedProfileIds: new Set() }),
  bumpProfiles: () => set((state) => ({ profilesVersion: state.profilesVersion + 1 })),
}));
