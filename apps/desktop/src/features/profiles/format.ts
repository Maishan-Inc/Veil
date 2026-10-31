import type { MessageKey } from "@veil/i18n";
import type { Profile } from "@veil/shared";

/** Letter shown inside an avatar circle. */
export function avatarInitial(name: string): string {
  const trimmed = name.trim();
  if (!trimmed) return "·";
  const first = trimmed.codePointAt(0);
  return first === undefined ? "·" : String.fromCodePoint(first).toUpperCase();
}

/** Localized OS name. */
export function osLabel(os: Profile["os"]): MessageKey {
  switch (os) {
    case "windows":
      return "os.windows";
    case "macos":
      return "os.macos";
    case "linux":
      return "os.linux";
    case "android":
      return "os.android";
    case "ios":
      return "os.ios";
  }
}

/** Formats an ISO timestamp as a short locale-aware string, or a fallback when null. */
export function formatLastLaunched(value: string | null, fallback: string): string {
  if (!value) return fallback;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return fallback;
  return date.toLocaleString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** Lowercased full-text needle that matches the visible columns of a profile. */
export function profileSearchBlob(profile: Profile): string {
  return [
    profile.name,
    profile.platform,
    profile.notes,
    profile.os,
    profile.browserVersion,
    profile.tagIds.join(" "),
  ]
    .join(" ")
    .toLowerCase();
}
