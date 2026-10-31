import { enUS } from "./locales/en-US";
import { jaJP } from "./locales/ja-JP";
import { ruRU } from "./locales/ru-RU";
import { zhCN } from "./locales/zh-CN";
import { zhTW } from "./locales/zh-TW";

export { enUS, jaJP, ruRU, zhCN, zhTW };

export type Locale = "en-US" | "zh-CN" | "zh-TW" | "ja-JP" | "ru-RU";

/** All message keys, derived from the English catalog (the source of truth). */
export type MessageKey = keyof typeof enUS;

export const locales: readonly Locale[] = ["en-US", "zh-CN", "zh-TW", "ja-JP", "ru-RU"];

const catalogs: Record<Locale, Record<string, string>> = {
  "en-US": enUS,
  "zh-CN": zhCN,
  "zh-TW": zhTW,
  "ja-JP": jaJP,
  "ru-RU": ruRU,
};

export type TParams = Record<string, string | number>;

/**
 * Translate a key for a locale, interpolating `{name}` placeholders.
 * Falls back to the key itself when it is missing (visible but non-crashing).
 */
export function t(locale: Locale, key: string, params?: TParams): string {
  const template = catalogs[locale][key] ?? key;
  if (!params) return template;
  return template.replace(/\{(\w+)\}/g, (_match, name: string) =>
    name in params ? String(params[name]) : `{${name}}`,
  );
}
