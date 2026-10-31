import { describe, expect, it } from "vitest";
import {
  generateRecoveryPhrase,
  pickVerificationIndexes,
  progressActive,
  recoveryPartInputValue,
  recoveryVerifyPartsFromPaste,
  verifyRecoveryPhrase,
} from "./onboardingHelpers";

describe("onboarding helpers", () => {
  it("generates a stable base32 recovery phrase format from mockable random bytes", () => {
    let next = 0;
    const phrase = generateRecoveryPhrase((bytes) => {
      bytes.fill(next);
      next += 1;
      return bytes;
    });

    expect(phrase).toHaveLength(12);
    expect(new Set(phrase).size).toBe(12);
    expect(phrase.every((part) => /^[A-Z2-7]{4}$/.test(part))).toBe(true);
    expect(phrase.slice(0, 3)).toEqual(["AAAA", "BBBB", "CCCC"]);
  });

  it("picks stable verification indexes and verifies only those parts", () => {
    const phrase = [
      "AAAA",
      "BBBB",
      "CCCC",
      "DDDD",
      "EEEE",
      "FFFF",
      "GGGG",
      "HHHH",
      "IIII",
      "JJJJ",
      "KKKK",
      "LLLL",
    ];
    const indexes = pickVerificationIndexes(phrase.length);

    expect(indexes).toEqual([1, 6, 10]);
    expect(
      verifyRecoveryPhrase(phrase, indexes, {
        1: "bbbb",
        6: " GGGG ",
        10: "kk-kk",
      }),
    ).toBe(true);
    expect(
      verifyRecoveryPhrase(phrase, indexes, {
        1: "BBBB",
        6: "GGGG",
        10: "LLLL",
      }),
    ).toBe(false);
  });

  it("normalizes pasted recovery phrase text into verification answers", () => {
    const phrase = [
      "AAAA",
      "BBBB",
      "CCCC",
      "DDDD",
      "EEEE",
      "FFFF",
      "GGGG",
      "HHHH",
      "IIII",
      "JJJJ",
      "KKKK",
      "LLLL",
    ];
    const indexes = pickVerificationIndexes(phrase.length);

    expect(recoveryPartInputValue("bbbb cccc")).toBe("BBBB");
    expect(
      recoveryVerifyPartsFromPaste(phrase, indexes, {}, indexes[0]!, phrase.join(" ")),
    ).toEqual({
      1: "BBBB",
      6: "GGGG",
      10: "KKKK",
    });
    expect(
      recoveryVerifyPartsFromPaste(phrase, indexes, {}, indexes[0]!, "bbbb gggg kkkk"),
    ).toEqual({
      1: "BBBB",
      6: "GGGG",
      10: "KKKK",
    });
  });

  it("keeps encryption, password, and 2FA under the Security progress step", () => {
    expect(progressActive("agreement", "agreement")).toBe(true);
    expect(progressActive("agreement", "security")).toBe(false);
    expect(progressActive("encryption", "security")).toBe(true);
    expect(progressActive("password", "security")).toBe(true);
    expect(progressActive("twoFactor", "security")).toBe(true);
    expect(progressActive("twoFactorVerify", "security")).toBe(true);
    expect(progressActive("twoFactorVerify", "recovery")).toBe(false);
    expect(progressActive("mnemonic", "recovery")).toBe(true);
    expect(progressActive("done", "done")).toBe(true);
  });
});
