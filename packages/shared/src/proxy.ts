import type { AppErrorCode } from "./errors";

export type ProxyType = "http" | "https" | "socks5";

export interface Proxy {
  id: string;
  name: string;
  type: ProxyType;
  host: string;
  port: number;
  username: string | null;
  /** The password itself never leaves the backend; only its presence is exposed. */
  hasPassword: boolean;
  country: string | null;
  detectedIp: string | null;
  countryCode: string | null;
  region: string | null;
  city: string | null;
  timezone: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface CreateProxyInput {
  name: string;
  type: ProxyType;
  host: string;
  port: number;
  username: string | null;
  password: string | null;
  country: string | null;
}

export interface ProxyTestResult {
  ok: boolean;
  ip: string | null;
  country: string | null;
  countryCode: string | null;
  region: string | null;
  city: string | null;
  timezone: string | null;
  latencyMs: number | null;
  code: AppErrorCode | null;
}
