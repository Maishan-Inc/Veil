import type { AppErrorCode } from "./errors";
import type { Fingerprint } from "./fingerprint";

/** Fingerprint operating system a profile presents. */
export type ProfileOS = "windows" | "macos" | "linux" | "android" | "ios";

/**
 * The values the OS picker offers, in order.
 *
 * Five enum values, three pickable ones. `android` and `ios` left the picker in T24
 * (T24 decided it, T24-a landed it): the patch set implements no
 * mobile platform, so the only honest thing a mobile selection could do was present the *host's*
 * platform, and a control that offered that value was the one part of the feature that lied.
 *
 * `ios` stays out (no precedent anywhere, no path to one — 6.24), and its variant survives here
 * only so a profile stored before the change keeps parsing, keeps serialising, and keeps launching
 * with the withholding `as_fingerprint_platform()` describes. `android` is back since the T24-a
 * port landed (6.26): the engine implements every surface the claim touches, `validate` requires
 * the phone's own coherence (a portrait panel, a scale factor, and a non-zero touch count), and
 * the preset derives all of them from the same device row the engine reads.
 */
export const SELECTABLE_PROFILE_OS = ["windows", "macos", "linux", "android"] as const;

/** Whether the OS picker offers this value. Mirrors {@link SELECTABLE_PROFILE_OS}. */
export function isSelectableProfileOs(os: ProfileOS): boolean {
  return (SELECTABLE_PROFILE_OS as readonly string[]).includes(os);
}

/** Live launch status of a profile's browser. */
export type ProfileStatus = "idle" | "launching" | "running" | "error";
export type StartupPage = "veil" | "google" | "bing" | "custom";

export interface Profile {
  id: string;
  name: string;
  /** Deterministic accent color seed for the avatar (hex). */
  avatarColor: string;
  groupId: string | null;
  tagIds: string[];
  proxyId: string | null;
  /** Fingerprint OS presented by the profile. */
  os: ProfileOS;
  /** Free-form use-case / site label (e.g. "Storefront", "Ads"). */
  platform: string;
  /** Engine/browser version this profile targets (e.g. "120"). */
  browserVersion: string;
  notes: string;
  status: ProfileStatus;
  startupPage: StartupPage;
  startupUrl: string;
  /** Fingerprint config mapped to engine launch flags by the backend. */
  fingerprint: Fingerprint;
  createdAt: string;
  updatedAt: string;
  lastLaunchedAt: string | null;
}

export interface Group {
  id: string;
  name: string;
  color: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface Tag {
  id: string;
  name: string;
  color: string;
}

export interface CreateProfileInput {
  name: string;
  groupId: string | null;
  tagIds: string[];
  proxyId: string | null;
  os: ProfileOS;
  platform: string;
  /** Installed kernel version this profile should launch with. */
  browserVersion: string;
  notes: string;
  startupPage: StartupPage;
  startupUrl: string;
  fingerprint: Fingerprint;
}

export type LaunchStatus =
  | { kind: "started"; profileId: string; processId: number }
  | { kind: "failed"; profileId: string; code: AppErrorCode };
