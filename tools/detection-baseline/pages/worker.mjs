/**
 * Module Worker driver. A Worker sees a smaller surface than a document (no `screen`, no DOM, no
 * `OfflineAudioContext`), and the probe records the gaps as `unavailable` rather than dropping them.
 */
import { collect } from "./probe.mjs";

collect("worker").then(
  (result) => postMessage({ veilRealm: "worker", result }),
  (error) =>
    postMessage({
      veilRealm: "worker",
      result: { realm: "worker", unavailable: `${error.name}: ${error.message}` },
    }),
);
