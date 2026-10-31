import { describe, expect, it } from "vitest";
import { parseProxyImportText } from "./proxyImport";

describe("parseProxyImportText", () => {
  it("parses one proxy per line across supported formats", () => {
    const result = parseProxyImportText(
      [
        "proxy.example.com:8080",
        "proxy.example.com:8081:user:pass",
        "8082:proxy.example.net:user2:pass2",
        "user3:pass3@proxy.example.org:8083",
        "http://user4:pass4@proxy.example.io:8084",
        "https://proxy.example.dev:8085",
        "socks5://proxy.example.local:8086:user6:pass6",
      ].join("\n"),
    );

    expect(result.invalid).toEqual([]);
    expect(result.valid.map((entry) => entry.input)).toMatchObject([
      {
        type: "http",
        host: "proxy.example.com",
        port: 8080,
        username: null,
        password: null,
      },
      {
        type: "http",
        host: "proxy.example.com",
        port: 8081,
        username: "user",
        password: "pass",
      },
      {
        type: "http",
        host: "proxy.example.net",
        port: 8082,
        username: "user2",
        password: "pass2",
      },
      {
        type: "http",
        host: "proxy.example.org",
        port: 8083,
        username: "user3",
        password: "pass3",
      },
      {
        type: "http",
        host: "proxy.example.io",
        port: 8084,
        username: "user4",
        password: "pass4",
      },
      {
        type: "https",
        host: "proxy.example.dev",
        port: 8085,
        username: null,
        password: null,
      },
      {
        type: "socks5",
        host: "proxy.example.local",
        port: 8086,
        username: "user6",
        password: "pass6",
      },
    ]);
  });

  it("keeps invalid lines with original line numbers", () => {
    const result = parseProxyImportText(
      ["proxy.example.com:8080", "missing-port", "host:99999", "bad host:8080"].join("\n"),
    );

    expect(result.valid).toHaveLength(1);
    expect(result.invalid).toEqual([
      { lineNumber: 2, raw: "missing-port" },
      { lineNumber: 3, raw: "host:99999" },
      { lineNumber: 4, raw: "bad host:8080" },
    ]);
  });
});
