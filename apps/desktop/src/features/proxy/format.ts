import type { Proxy } from "@veil/shared";

export function countryFlag(countryCode: string | null): string | null {
  const code = countryCode?.trim().toUpperCase();
  if (!code || !/^[A-Z]{2}$/.test(code)) return null;
  return String.fromCodePoint(...[...code].map((char) => 127397 + char.charCodeAt(0)));
}

export function formatProxyLocation(proxy: Proxy): string | null {
  const parts: string[] = [];
  const flag = countryFlag(proxy.countryCode);
  if (flag) parts.push(flag);
  if (proxy.detectedIp) parts.push(proxy.detectedIp);
  const country = proxy.country ?? proxy.countryCode;
  if (country) parts.push(country);
  const area = proxy.city ?? proxy.region;
  if (area && area !== country) parts.push(area);
  return parts.length ? parts.join(" · ") : null;
}

export function formatProxyOption(proxy: Proxy): string {
  const location = formatProxyLocation(proxy);
  return location ? `${proxy.name} - ${location}` : proxy.name;
}
