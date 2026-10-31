/**
 * The fixture origins. The pages the probe runs in are served from loopback rather than fetched from
 * the internet, which makes the capture fully offline *and* deterministic: no third-party site can
 * rescore, redirect, or A/B the thing we are supposed to be diffing.
 *
 * Two servers, because the iframe realm has to be a genuinely separate one:
 *
 * - main page → `http://127.0.0.1:<port>`
 * - iframe    → `http://localhost:<port>`
 *
 * `localhost` and `127.0.0.1` are different *sites* to Chromium, not merely different origins, so
 * the frame lands in its own renderer process. Same-port-different-path would share a realm and the
 * cross-realm check would be worthless.
 *
 * Both hostnames are secure contexts, which the probe needs for `crypto.subtle`,
 * `navigator.userAgentData.getHighEntropyValues` and `navigator.keyboard`.
 */

import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join, normalize, resolve, sep } from "node:path";

const CONTENT_TYPES = {
  ".html": "text/html; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
};

async function startServer({ host, pagesDir, replacements, port = 0 }) {
  const root = resolve(pagesDir);
  const server = createServer(async (request, response) => {
    try {
      const requested = new URL(request.url, "http://placeholder").pathname;
      const target = resolve(join(root, normalize(requested)));
      if (target !== root && !target.startsWith(root + sep)) {
        response.writeHead(403).end("outside the fixture root");
        return;
      }
      let body = await readFile(target, "utf8");
      for (const [token, value] of Object.entries(replacements)) {
        body = body.split(token).join(value);
      }
      response.writeHead(200, {
        "content-type": CONTENT_TYPES[extname(target)] ?? "application/octet-stream",
        // A fixture must never be answered from a previous run's cache.
        "cache-control": "no-store",
      });
      response.end(body);
    } catch (error) {
      response.writeHead(error.code === "ENOENT" ? 404 : 500).end(String(error.message));
    }
  });
  await new Promise((ok, fail) => {
    server.once("error", fail);
    server.listen({ host, port }, ok);
  });
  return { server, port: server.address().port };
}

export async function startFixtures(pagesDir) {
  // The frame server binds IPv4 loopback; a best-effort second listener on IPv6 loopback covers
  // machines where Chromium resolves `localhost` to `::1` first. A failure there is not fatal —
  // Chromium falls back to the other family — so it is logged, not thrown.
  const frame = await startServer({ host: "127.0.0.1", pagesDir, replacements: {} });
  let frameV6 = null;
  try {
    frameV6 = await startServer({ host: "::1", pagesDir, replacements: {}, port: frame.port });
  } catch (error) {
    process.stderr.write(`note: no IPv6 loopback listener for the frame origin (${error.code})\n`);
  }
  const frameOrigin = `http://localhost:${frame.port}`;
  const main = await startServer({
    host: "127.0.0.1",
    pagesDir,
    replacements: { __FRAME_ORIGIN__: frameOrigin },
  });
  const mainOrigin = `http://127.0.0.1:${main.port}`;

  // Fail loudly here rather than let the capture report an empty iframe realm and look like a
  // finding about the engine.
  for (const url of [`${mainOrigin}/main.html`, `${frameOrigin}/frame.html`]) {
    const response = await fetch(url);
    if (!response.ok) {
      throw new Error(`fixture ${url} answered ${response.status}`);
    }
  }

  return {
    mainUrl: `${mainOrigin}/main.html`,
    mainOrigin,
    frameOrigin,
    ports: [main.port, frame.port],
    async close() {
      for (const instance of [main.server, frame.server, frameV6?.server]) {
        if (instance) {
          await new Promise((ok) => instance.close(ok));
        }
      }
    },
  };
}
