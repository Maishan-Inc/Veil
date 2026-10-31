import { createContext, createElement, useContext, useMemo, type ReactNode } from "react";
import { t as translate, type Locale, type TParams } from "./catalog";

export interface I18nContextValue {
  locale: Locale;
  t: (key: string, params?: TParams) => string;
}

const I18nContext = createContext<I18nContextValue | null>(null);

export function I18nProvider({
  locale,
  children,
}: {
  locale: Locale;
  children: ReactNode;
}): ReactNode {
  const value = useMemo<I18nContextValue>(
    () => ({ locale, t: (key, params) => translate(locale, key, params) }),
    [locale],
  );
  return createElement(I18nContext.Provider, { value }, children);
}

export function useI18n(): I18nContextValue {
  const ctx = useContext(I18nContext);
  if (!ctx) {
    throw new Error("useI18n must be used within an I18nProvider");
  }
  return ctx;
}

/** Convenience hook returning just the translate function. */
export function useT(): (key: string, params?: TParams) => string {
  return useI18n().t;
}
