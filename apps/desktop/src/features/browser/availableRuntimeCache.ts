import type { AvailableRuntime } from "@veil/shared";

const CACHE_KEY = "veil.browser.availableRuntimes.v1";

let cachedAvailableRuntimes: AvailableRuntime[] | null = null;

export function getCachedAvailableRuntimes(): AvailableRuntime[] | null {
  if (!cachedAvailableRuntimes) {
    cachedAvailableRuntimes = readStoredAvailableRuntimes();
  }
  return cachedAvailableRuntimes ? [...cachedAvailableRuntimes] : null;
}

export function rememberAvailableRuntimes(list: AvailableRuntime[]): void {
  cachedAvailableRuntimes = [...list];
  try {
    globalThis.localStorage?.setItem(CACHE_KEY, JSON.stringify(cachedAvailableRuntimes));
  } catch {
    // Cache persistence is best-effort; the in-memory list still protects this session.
  }
}

export function clearAvailableRuntimeCacheForTests(): void {
  cachedAvailableRuntimes = null;
  try {
    globalThis.localStorage?.removeItem(CACHE_KEY);
  } catch {
    // ignored in non-browser test environments
  }
}

function readStoredAvailableRuntimes(): AvailableRuntime[] | null {
  try {
    const raw = globalThis.localStorage?.getItem(CACHE_KEY);
    if (!raw) return null;
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed) || !parsed.every(isAvailableRuntime)) return null;
    return parsed;
  } catch {
    return null;
  }
}

function isAvailableRuntime(value: unknown): value is AvailableRuntime {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<AvailableRuntime>;
  return (
    typeof candidate.version === "string" &&
    typeof candidate.assetName === "string" &&
    typeof candidate.downloadUrl === "string" &&
    typeof candidate.sha256 === "string" &&
    (typeof candidate.sizeBytes === "number" || candidate.sizeBytes === null) &&
    (typeof candidate.publishedAt === "string" || candidate.publishedAt === null)
  );
}
