/**
 * Cross-site frame driver. Served from a different registrable domain than the main page
 * (`localhost` vs `127.0.0.1`), so Chromium puts it in its own renderer process — the realm split
 * that matters for assertion 2.
 */
import { collect } from "./probe.mjs";

collect("iframe").then(
  (result) => parent.postMessage({ veilRealm: "iframe", result }, "*"),
  (error) =>
    parent.postMessage(
      {
        veilRealm: "iframe",
        result: { realm: "iframe", unavailable: `${error.name}: ${error.message}` },
      },
      "*",
    ),
);
