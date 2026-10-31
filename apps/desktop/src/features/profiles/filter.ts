import type { Profile, ProfileStatus } from "@veil/shared";
import { profileSearchBlob } from "./format";

export type StatusFilter = "all" | ProfileStatus;
export type SortKey = "name" | "browserVersion" | "platform" | "lastLaunchedAt" | "status";
export type SortDir = "asc" | "desc";

export interface FilterState {
  search: string;
  status: StatusFilter;
  sortKey: SortKey;
  sortDir: SortDir;
}

/** Apply the active search/status filter and sort. Pure. */
export function applyFilters(profiles: Profile[], filters: FilterState): Profile[] {
  const needle = filters.search.trim().toLowerCase();
  const filtered = profiles.filter((profile) => {
    if (filters.status !== "all" && profile.status !== filters.status) return false;
    if (!needle) return true;
    return profileSearchBlob(profile).includes(needle);
  });

  filtered.sort((a, b) => compare(a, b, filters.sortKey) * (filters.sortDir === "asc" ? 1 : -1));
  return filtered;
}

function compare(a: Profile, b: Profile, key: SortKey): number {
  switch (key) {
    case "name":
      return a.name.localeCompare(b.name);
    case "browserVersion":
      return (a.browserVersion ?? "").localeCompare(b.browserVersion ?? "");
    case "platform":
      return (a.platform ?? "").localeCompare(b.platform ?? "");
    case "status":
      return STATUS_ORDER[a.status] - STATUS_ORDER[b.status];
    case "lastLaunchedAt": {
      const av = a.lastLaunchedAt ?? "";
      const bv = b.lastLaunchedAt ?? "";
      // Empty strings sort first ascending, so "Never" rows end up at the start in asc.
      return av.localeCompare(bv);
    }
  }
}

const STATUS_ORDER: Record<ProfileStatus, number> = {
  running: 0,
  launching: 1,
  idle: 2,
  error: 3,
};
