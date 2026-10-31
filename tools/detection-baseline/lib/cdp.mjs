/**
 * Minimal CDP client for the detection baseline. Read-only **by construction**, not by convention.
 *
 * Implementing fingerprint behaviour in this repo is forbidden, and spoofing a value from
 * the automation layer is precisely the anti-pattern the ungoogled migration exists to remove. Two
 * guards make that structural rather than aspirational:
 *
 * 1. `ALLOWED_METHODS` — the only CDP methods this client will send. `Emulation.*`,
 *    `Network.setUserAgentOverride` and `Page.addScriptToEvaluateOnNewDocument` are not on it, so
 *    reaching for one is a thrown error at the call site instead of a code review comment.
 * 2. `ALLOWED_EXPRESSIONS` — `Runtime.evaluate` takes arbitrary JavaScript, which would be a hole in
 *    guard 1. So the client refuses to evaluate anything but the two fixed expressions that read the
 *    result the fixture page computed for itself.
 */

const ALLOWED_METHODS = new Set([
  "Browser.close",
  "Target.createTarget",
  "Target.attachToTarget",
  "Target.closeTarget",
  "Runtime.evaluate",
]);

export const EXPR_STATUS = "window.__veilBaselineStatus ?? null";
export const EXPR_RESULT = "JSON.stringify(window.__veilBaseline)";
const ALLOWED_EXPRESSIONS = new Set([EXPR_STATUS, EXPR_RESULT]);

class ProtocolError extends Error {}

export class CdpClient {
  #socket;
  #nextId = 1;
  #pending = new Map();

  constructor(socket) {
    this.#socket = socket;
    socket.addEventListener("message", (event) => this.#onMessage(event.data));
    socket.addEventListener("close", () => {
      for (const { reject } of this.#pending.values()) {
        reject(new ProtocolError("CDP socket closed"));
      }
      this.#pending.clear();
    });
  }

  static async connect(httpBase) {
    const response = await fetch(new URL("/json/version", httpBase));
    if (!response.ok) {
      throw new ProtocolError(`GET /json/version returned ${response.status}`);
    }
    const version = await response.json();
    const url = version.webSocketDebuggerUrl;
    if (!url) {
      throw new ProtocolError("DevTools endpoint did not advertise webSocketDebuggerUrl");
    }
    const socket = new WebSocket(url);
    await new Promise((resolve, reject) => {
      socket.addEventListener("open", resolve, { once: true });
      socket.addEventListener("error", () => reject(new ProtocolError(`cannot open ${url}`)), {
        once: true,
      });
    });
    return { client: new CdpClient(socket), version };
  }

  #onMessage(raw) {
    const message = JSON.parse(typeof raw === "string" ? raw : raw.toString());
    // Protocol *events* are ignored on purpose: the fixture page signals completion through a value
    // this client polls, so there is no event subscription and nothing to keep in sync.
    if (message.id === undefined || !this.#pending.has(message.id)) {
      return;
    }
    const { resolve, reject } = this.#pending.get(message.id);
    this.#pending.delete(message.id);
    if (message.error) {
      reject(new ProtocolError(`${message.error.message} (code ${message.error.code})`));
    } else {
      resolve(message.result);
    }
  }

  send(method, params = {}, sessionId) {
    if (!ALLOWED_METHODS.has(method)) {
      throw new ProtocolError(
        `${method} is not on the detection-baseline allowlist. The harness reads page state; ` +
          "changing what the page observes belongs in a Veil-Chromium patch.",
      );
    }
    if (method === "Runtime.evaluate" && !ALLOWED_EXPRESSIONS.has(params.expression)) {
      throw new ProtocolError(
        "Runtime.evaluate is restricted to reading window.__veilBaseline; the fixture page owns " +
          "every value that is measured.",
      );
    }
    const id = this.#nextId++;
    const payload = { id, method, params };
    if (sessionId) {
      payload.sessionId = sessionId;
    }
    return new Promise((resolve, reject) => {
      this.#pending.set(id, { resolve, reject });
      this.#socket.send(JSON.stringify(payload));
    });
  }

  /** Evaluate one of the two allowlisted read expressions and return its value. */
  async read(expression, sessionId) {
    const result = await this.send(
      "Runtime.evaluate",
      { expression, returnByValue: true, awaitPromise: false },
      sessionId,
    );
    if (result.exceptionDetails) {
      throw new ProtocolError(`evaluate threw: ${result.exceptionDetails.text}`);
    }
    return result.result.value;
  }

  close() {
    this.#socket.close();
  }
}
