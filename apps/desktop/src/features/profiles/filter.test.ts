import { describe, expect, it } from "vitest";
import { emptyFingerprint, type Profile } from "@veil/shared";
import { applyFilters } from "./filter";

const make = (overrides: Partial<Profile>): Profile => ({
  id: overrides.id ?? "p",
  name: overrides.name ?? "P",
  avatarColor: "#000",
  groupId: null,
  tagIds: [],
  proxyId: null,
  os: "windows",
  platform: "",
  browserVersion: "120",
  notes: "",
  status: overrides.status ?? "idle",
  startupPage: overrides.startupPage ?? "veil",
  startupUrl: overrides.startupUrl ?? "",
  fingerprint: emptyFingerprint(),
  createdAt: "2026-06-27T00:00:00Z",
  updatedAt: "2026-06-27T00:00:00Z",
  lastLaunchedAt: overrides.lastLaunchedAt ?? null,
  ...overrides,
});

describe("applyFilters", () => {
  const profiles: Profile[] = [
    make({ id: "a", name: "Storefront", status: "running" }),
    make({ id: "b", name: "Ads Manager", status: "idle" }),
    make({ id: "c", name: "Research", status: "idle", platform: "Research EU" }),
  ];

  it("filters by status", () => {
    const result = applyFilters(profiles, {
      search: "",
      status: "running",
      sortKey: "name",
      sortDir: "asc",
    });
    expect(result.map((p) => p.id)).toEqual(["a"]);
  });

  it("filters by case-insensitive search across visible columns", () => {
    const result = applyFilters(profiles, {
      search: "EU",
      status: "all",
      sortKey: "name",
      sortDir: "asc",
    });
    expect(result.map((p) => p.id)).toEqual(["c"]);
  });

  it("sorts by name descending", () => {
    const result = applyFilters(profiles, {
      search: "",
      status: "all",
      sortKey: "name",
      sortDir: "desc",
    });
    expect(result.map((p) => p.name)).toEqual(["Storefront", "Research", "Ads Manager"]);
  });

  it("sorts by status with running first when ascending", () => {
    const result = applyFilters(profiles, {
      search: "",
      status: "all",
      sortKey: "status",
      sortDir: "asc",
    });
    expect(result[0]?.status).toBe("running");
  });
});
