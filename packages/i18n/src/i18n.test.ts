import { describe, expect, it } from "vitest";
import { enUS, jaJP, locales, ruRU, t, zhCN, zhTW } from "./catalog";

describe("i18n catalogs", () => {
  it("keeps every locale in key parity with en-US", () => {
    const reference = Object.keys(enUS).sort();
    for (const [locale, catalog] of Object.entries({ zhCN, zhTW, jaJP, ruRU })) {
      expect(Object.keys(catalog).sort(), locale).toEqual(reference);
    }
  });

  it("exposes the supported locales", () => {
    expect(locales).toEqual(["en-US", "zh-CN", "zh-TW", "ja-JP", "ru-RU"]);
  });

  it("translates a known key across locales", () => {
    expect(t("en-US", "errors.runtime.checksumMismatch")).toContain("checksum");
    expect(t("zh-CN", "errors.runtime.checksumMismatch")).toContain("校验");
    expect(t("zh-TW", "settings.language.zh-TW")).toContain("繁體");
    expect(t("ja-JP", "errors.runtime.checksumMismatch")).toContain("チェックサム");
    expect(t("ru-RU", "errors.runtime.checksumMismatch")).toContain("суммы");
  });

  it("interpolates parameters", () => {
    expect(t("en-US", "profiles.selected", { count: 3 })).toBe("3 selected");
    expect(t("zh-CN", "status.engine.installed", { version: "120" })).toBe("内核 120");
  });

  it("carries a string for every fingerprint field note", () => {
    // `@veil/shared`'s `fingerprintFieldState` hands the View these keys; the counterpart
    // assertion there pins the key strings. A missing entry here would render the raw key.
    // `en-US` and `zh-CN` are the parity pair, so both must be real translations.
    const notes = [
      "fingerprint.field.seedOwnsUserAgent",
      "fingerprint.field.unsupported",
      "fingerprint.field.displayNeedsSeed",
      "fingerprint.field.displayNeedsPlatform",
      "fingerprint.field.window.hint",
      "fingerprint.field.spoofedNotConfigurable.canvasNoise",
      "fingerprint.field.spoofedNotConfigurable.audioNoise",
      // Phase 4's two cross-field notes. `spoofedNotConfigurable.deviceMemory` left this list with
      // T15: `905` reads the stored value, so the field is settable and has no note at all.
      "fingerprint.field.maxTouchPoints.host",
      "fingerprint.field.maxTouchPoints.macos",
      "fingerprint.field.deviceMemory.host",
    ];
    for (const key of notes) {
      expect(t("en-US", key), key).not.toBe(key);
      expect(t("zh-CN", key), key).not.toBe(key);
      expect(t("zh-CN", key), key).not.toBe(t("en-US", key));
    }
  });

  it("falls back to the key when it is missing", () => {
    expect(t("en-US", "this.key.does.not.exist")).toBe("this.key.does.not.exist");
  });
});
