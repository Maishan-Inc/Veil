/**
 * Main-frame driver. Collects this realm, asks the cross-site frame and the module Worker for
 * theirs, and parks the three results on `window` for the capture tool to read.
 *
 * The capture tool only ever *reads* `window.__veilBaseline`. All of the collecting happens here, in
 * ordinary page script served from the fixture, because a harness that injected its probe over CDP
 * would be doing the very thing this migration exists to avoid.
 */
import { collect } from "./probe.mjs";

const REALM_TIMEOUT_MS = 30000;

window.__veilBaselineStatus = "running";

function awaitRealm(name, start) {
  return new Promise((resolve) => {
    const timer = setTimeout(() => {
      window.removeEventListener("message", onMessage);
      resolve({ realm: name, unavailable: `timed out after ${REALM_TIMEOUT_MS}ms` });
    }, REALM_TIMEOUT_MS);
    function onMessage(event) {
      if (!event.data || event.data.veilRealm !== name) {
        return;
      }
      clearTimeout(timer);
      window.removeEventListener("message", onMessage);
      resolve(event.data.result);
    }
    window.addEventListener("message", onMessage);
    try {
      start(onMessage);
    } catch (error) {
      clearTimeout(timer);
      window.removeEventListener("message", onMessage);
      resolve({ realm: name, unavailable: `${error.name}: ${error.message}` });
    }
  });
}

async function run() {
  const iframe = awaitRealm("iframe", () => {
    // frame.html posts as soon as it is done; nothing to kick off here beyond having the element.
    if (!document.getElementById("veil-frame")) {
      throw new Error("fixture iframe missing");
    }
  });
  const worker = awaitRealm("worker", () => {
    const instance = new Worker("/worker.mjs", { type: "module" });
    instance.addEventListener("message", (event) =>
      window.postMessage(event.data, location.origin),
    );
    instance.addEventListener("error", (event) =>
      window.postMessage(
        {
          veilRealm: "worker",
          result: { realm: "worker", unavailable: `worker error: ${event.message}` },
        },
        location.origin,
      ),
    );
  });

  const main = await collect("main");
  window.__veilBaseline = {
    main,
    iframe: await iframe,
    worker: await worker,
  };
  window.__veilBaselineStatus = "done";
}

run().catch((error) => {
  window.__veilBaseline = { error: `${error.name}: ${error.message}` };
  window.__veilBaselineStatus = "failed";
});
