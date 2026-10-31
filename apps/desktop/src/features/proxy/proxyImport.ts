import type { CreateProxyInput, ProxyType } from "@veil/shared";

export interface ParsedProxyLine {
  lineNumber: number;
  raw: string;
  input: CreateProxyInput;
}

export interface InvalidProxyLine {
  lineNumber: number;
  raw: string;
}

export interface ProxyImportParseResult {
  valid: ParsedProxyLine[];
  invalid: InvalidProxyLine[];
}

const SUPPORTED_PROTOCOLS = new Set<ProxyType>(["http", "https", "socks5"]);

export function parseProxyImportText(text: string): ProxyImportParseResult {
  const valid: ParsedProxyLine[] = [];
  const invalid: InvalidProxyLine[] = [];

  text.split(/\r?\n/).forEach((rawLine, index) => {
    const raw = rawLine.trim();
    if (!raw) return;
    const input = parseProxyLine(raw);
    if (input) {
      valid.push({ lineNumber: index + 1, raw, input });
    } else {
      invalid.push({ lineNumber: index + 1, raw });
    }
  });

  return { valid, invalid };
}

function parseProxyLine(raw: string): CreateProxyInput | null {
  const urlParsed = parseUrlLikeLine(raw);
  if (urlParsed) return urlParsed;

  const authParsed = parseUserPassAtHostPort(raw, "http");
  if (authParsed) return authParsed;

  const parts = raw.split(":");
  if (parts.length === 2) {
    const host = parts[0]!;
    const port = parts[1]!;
    return buildInput({
      type: "http",
      host,
      port,
      username: null,
      password: null,
    });
  }

  if (parts.length === 4) {
    const a = parts[0]!;
    const b = parts[1]!;
    const c = parts[2]!;
    const d = parts[3]!;
    if (isValidPort(a) && !isValidPort(b)) {
      return buildInput({
        type: "http",
        host: b,
        port: a,
        username: c,
        password: d,
      });
    }
    return buildInput({
      type: "http",
      host: a,
      port: b,
      username: c,
      password: d,
    });
  }

  return null;
}

function parseUrlLikeLine(raw: string): CreateProxyInput | null {
  const protocolMatch = raw.match(/^([a-z0-9+.-]+):\/\//i);
  if (!protocolMatch) return null;
  const protocol = protocolMatch[1]!.toLowerCase();
  if (!SUPPORTED_PROTOCOLS.has(protocol as ProxyType)) return null;
  const type = protocol as ProxyType;

  try {
    const url = new URL(raw);
    if (url.hostname && url.port) {
      return buildInput({
        type,
        host: url.hostname,
        port: url.port,
        username: decodeURIComponent(url.username) || null,
        password: decodeURIComponent(url.password) || null,
      });
    }
  } catch {
    // Fall through to the loose parser below. URL rejects a few proxy-list formats
    // that users paste in practice, such as socks5://host:port:user:pass.
  }

  const withoutProtocol = raw.slice(protocolMatch[0].length);
  return (
    parseUserPassAtHostPort(withoutProtocol, type) ?? parseLooseColonLine(withoutProtocol, type)
  );
}

function parseUserPassAtHostPort(raw: string, type: ProxyType): CreateProxyInput | null {
  const atIndex = raw.lastIndexOf("@");
  if (atIndex <= 0 || atIndex === raw.length - 1) return null;

  const auth = raw.slice(0, atIndex);
  const hostPort = raw.slice(atIndex + 1);
  const authParts = auth.split(":");
  const hostParts = hostPort.split(":");
  if (authParts.length !== 2 || hostParts.length !== 2) return null;
  const username = authParts[0]!;
  const password = authParts[1]!;
  const host = hostParts[0]!;
  const port = hostParts[1]!;

  return buildInput({
    type,
    host,
    port,
    username,
    password,
  });
}

function parseLooseColonLine(raw: string, type: ProxyType): CreateProxyInput | null {
  const parts = raw.split(":");
  if (parts.length === 2) {
    const host = parts[0]!;
    const port = parts[1]!;
    return buildInput({
      type,
      host,
      port,
      username: null,
      password: null,
    });
  }
  if (parts.length === 4) {
    const a = parts[0]!;
    const b = parts[1]!;
    const c = parts[2]!;
    const d = parts[3]!;
    if (isValidPort(a) && !isValidPort(b)) {
      return buildInput({ type, host: b, port: a, username: c, password: d });
    }
    return buildInput({ type, host: a, port: b, username: c, password: d });
  }
  return null;
}

function buildInput(input: {
  type: ProxyType;
  host: string | null;
  port: string | null;
  username: string | null;
  password: string | null;
}): CreateProxyInput | null {
  const host = input.host?.trim() ?? "";
  const port = parsePort(input.port ?? "");
  if (!host || port === null || hasWhitespace(host)) return null;

  const username = normalizeOptional(input.username);
  const password = normalizeOptional(input.password);
  return {
    name: `${host}:${port}`,
    type: input.type,
    host,
    port,
    username,
    password,
    country: null,
  };
}

function normalizeOptional(value: string | null): string | null {
  const trimmed = value?.trim() ?? "";
  return trimmed ? trimmed : null;
}

function parsePort(value: string): number | null {
  const trimmed = value.trim();
  if (!isValidPort(trimmed)) return null;
  return Number.parseInt(trimmed, 10);
}

function isValidPort(value: string | undefined): boolean {
  if (!value || !/^\d+$/.test(value.trim())) return false;
  const port = Number.parseInt(value, 10);
  return port > 0 && port <= 65535;
}

function hasWhitespace(value: string): boolean {
  return /\s/.test(value);
}
