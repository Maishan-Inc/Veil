export type OnboardingStep =
  | "splash"
  | "agreement"
  | "encryption"
  | "password"
  | "twoFactor"
  | "twoFactorVerify"
  | "mnemonic"
  | "mnemonicVerify"
  | "done";

export type ProgressStepId = "agreement" | "security" | "recovery" | "done";

export type RandomBytes = (bytes: Uint8Array) => Uint8Array;

const RECOVERY_PART_COUNT = 12;
const RECOVERY_PART_LENGTH = 4;
const BASE32_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

const stepProgressIndex: Record<OnboardingStep, number> = {
  splash: -1,
  agreement: 0,
  encryption: 1,
  password: 1,
  twoFactor: 1,
  twoFactorVerify: 1,
  mnemonic: 2,
  mnemonicVerify: 2,
  done: 3,
};

const progressStepIndex: Record<ProgressStepId, number> = {
  agreement: 0,
  security: 1,
  recovery: 2,
  done: 3,
};

export function progressActive(current: OnboardingStep, target: ProgressStepId): boolean {
  return stepProgressIndex[current] >= progressStepIndex[target];
}

export function generateRecoveryPhrase(randomBytes: RandomBytes = fillRandomBytes): string[] {
  const parts: string[] = [];
  const seen = new Set<string>();
  while (parts.length < RECOVERY_PART_COUNT) {
    const part = recoveryPart(randomBytes);
    if (!seen.has(part)) {
      seen.add(part);
      parts.push(part);
    }
  }
  return parts;
}

export function pickVerificationIndexes(partCount: number, count = 3): number[] {
  if (partCount <= 0 || count <= 0) return [];

  const preferred = [1, Math.floor(partCount / 2), Math.max(0, partCount - 2)];
  const indexes: number[] = [];

  for (const candidate of preferred) {
    const bounded = Math.min(Math.max(candidate, 0), partCount - 1);
    if (!indexes.includes(bounded)) indexes.push(bounded);
    if (indexes.length === count) return indexes;
  }

  for (let index = 0; index < partCount && indexes.length < count; index += 1) {
    if (!indexes.includes(index)) indexes.push(index);
  }

  return indexes;
}

export function verifyRecoveryPhrase(
  phrase: readonly string[],
  indexes: readonly number[],
  answers: Readonly<Record<number, string>>,
): boolean {
  return indexes.every((index) => normalizeRecoveryPart(answers[index] ?? "") === phrase[index]);
}

export function normalizeRecoveryPart(value: string): string {
  return value.replace(/[\s-]/g, "").trim().toUpperCase();
}

export function recoveryPartsFromText(value: string): string[] {
  const parts = value
    .split(/[\s,，]+/)
    .map(normalizeRecoveryPart)
    .filter(Boolean);
  const compactPart = parts[0] ?? "";
  if (parts.length === 1 && compactPart.length > RECOVERY_PART_LENGTH) {
    const chunks: string[] = [];
    for (let offset = 0; offset < compactPart.length; offset += RECOVERY_PART_LENGTH) {
      chunks.push(compactPart.slice(offset, offset + RECOVERY_PART_LENGTH));
    }
    return chunks;
  }
  return parts;
}

export function recoveryPartInputValue(value: string): string {
  return normalizeRecoveryPart(value).slice(0, RECOVERY_PART_LENGTH);
}

export function recoveryVerifyPartsFromPaste(
  phrase: readonly string[],
  indexes: readonly number[],
  current: Readonly<Record<number, string>>,
  targetIndex: number,
  pastedText: string,
): Record<number, string> {
  const pastedParts = recoveryPartsFromText(pastedText);
  if (pastedParts.length === 0) return { ...current };

  const next = { ...current };
  if (pastedParts.length === phrase.length) {
    indexes.forEach((index) => {
      next[index] = recoveryPartInputValue(pastedParts[index] ?? "");
    });
    return next;
  }

  const start = Math.max(0, indexes.indexOf(targetIndex));
  indexes.slice(start).forEach((index, offset) => {
    const part = pastedParts[offset];
    if (part) next[index] = recoveryPartInputValue(part);
  });
  return next;
}

function fillRandomBytes(bytes: Uint8Array): Uint8Array {
  globalThis.crypto.getRandomValues(bytes);
  return bytes;
}

function recoveryPart(randomBytes: RandomBytes): string {
  const bytes = randomBytes(new Uint8Array(RECOVERY_PART_LENGTH));
  return Array.from(bytes, (byte) => BASE32_ALPHABET[byte & 31]).join("");
}
