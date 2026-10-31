const MAX_STEPS = 20;
const MAX_TEXT = 7000;
const CDP_RETRY_MS = 30_000;

main().catch((error) => {
  emit({ type: "event", kind: "error", message: shortMessage(error) });
  emit({
    type: "result",
    status: "failed",
    message: "Automation worker failed before it could finish the task.",
  });
  process.exitCode = 1;
});

async function main() {
  const request = JSON.parse(await readStdin());
  if (request.kind !== "runTask") {
    throw new Error("Unsupported worker request.");
  }
  await runTask(request);
}

async function runTask(request) {
  const page = await connectToPage(request.cdpUrl);
  emit({ type: "event", kind: "info", message: "Connected to the browser automation endpoint." });

  try {
    let snapshot = await takeSnapshot(page);
    const messages = buildInitialMessages(request, snapshot);
    let verificationAttempted = false;

    for (let step = 0; step < MAX_STEPS; step += 1) {
      const gate = classifyPage(snapshot);
      if (gate === "payment") {
        emitResult("pausedForUser", "Payment details require user action.", "payment.required");
        return;
      }
      if (gate === "humanVerification") {
        if (request.executionMode === "fullAuto" && !verificationAttempted) {
          verificationAttempted = true;
          emit({
            type: "event",
            kind: "warning",
            message: "Human verification appears to be present. Waiting once before pausing.",
          });
          await delay(3500);
          snapshot = await takeSnapshot(page);
          continue;
        }
        emitResult(
          "pausedForUser",
          "Human verification requires user action in the browser.",
          "human.verification",
        );
        return;
      }

      const assistant = await callModel(request.provider, messages);
      const message = assistant.message ?? {};
      const toolCalls = Array.isArray(message.tool_calls) ? message.tool_calls : [];

      if (toolCalls.length === 0) {
        const content = typeof message.content === "string" ? message.content.trim() : "";
        if (content.length > 0) {
          emit({ type: "event", kind: "info", message: truncate(content, 700) });
        }
        emitResult("completed", "AI task completed.");
        return;
      }

      messages.push({
        role: "assistant",
        content: typeof message.content === "string" ? message.content : "",
        tool_calls: toolCalls,
      });

      for (const call of toolCalls) {
        const name = call.function?.name;
        const args = parseToolArgs(call.function?.arguments);

        if (name === "done") {
          emitResult("completed", stringArg(args.message) || "AI task completed.");
          return;
        }
        if (name === "pause_for_user") {
          emitResult(
            "pausedForUser",
            stringArg(args.message) || "The AI paused for user action.",
            stringArg(args.code) || "user.required",
          );
          return;
        }

        const output = await runTool(page, name, args);
        emit({ type: "event", kind: output.kind, message: output.message });
        if (output.snapshot) {
          snapshot = output.snapshot;
        } else if (name !== "snapshot") {
          snapshot = await takeSnapshot(page);
        }

        messages.push({
          role: "tool",
          tool_call_id: call.id,
          content: JSON.stringify(output.data ?? { ok: true }).slice(0, MAX_TEXT),
        });
      }
    }

    emitResult("failed", "Automation stopped after reaching the step limit.");
  } finally {
    page.close();
  }
}

async function runTool(page, name, args) {
  switch (name) {
    case "navigate": {
      const url = normalizeUrl(requiredString(args.url, "url"));
      await page.send("Page.navigate", { url });
      await delay(2500);
      const snapshot = await takeSnapshot(page);
      return {
        kind: "info",
        message: `Navigated to ${snapshot.url || url}.`,
        data: snapshot,
        snapshot,
      };
    }
    case "click": {
      const result = await page.evaluate(clickExpression(args));
      await delay(900);
      return {
        kind: result.ok ? "info" : "warning",
        message: result.ok
          ? `Clicked ${result.label}.`
          : "Could not find a matching clickable element.",
        data: result,
      };
    }
    case "type_text": {
      const result = await page.evaluate(typeExpression(args));
      await delay(500);
      return {
        kind: result.ok ? "info" : "warning",
        message: result.ok ? `Filled ${result.label}.` : "Could not find a matching input field.",
        data: result,
      };
    }
    case "wait": {
      const ms = clamp(Number(args.ms ?? 1000), 200, 8000);
      await delay(ms);
      const snapshot = await takeSnapshot(page);
      return {
        kind: "info",
        message: `Waited ${ms} ms.`,
        data: snapshot,
        snapshot,
      };
    }
    case "snapshot": {
      const snapshot = await takeSnapshot(page);
      return {
        kind: "info",
        message: `Read page snapshot: ${snapshot.title || snapshot.url || "untitled page"}.`,
        data: snapshot,
        snapshot,
      };
    }
    default:
      return {
        kind: "warning",
        message: `Ignored unsupported tool: ${String(name || "unknown")}.`,
        data: { ok: false, error: "unsupported_tool" },
      };
  }
}

function buildInitialMessages(request, snapshot) {
  const facts = Array.isArray(request.facts)
    ? request.facts.filter((fact) => !/pass(word)?|pwd|token|secret/i.test(String(fact.key)))
    : [];
  const factLines = facts.length
    ? facts.map((fact) => `- ${fact.key}: ${fact.value}`).join("\n")
    : "- No profile facts are available.";

  const system = [
    "You control one user-launched browser profile through restricted tools.",
    "Use only the provided tools. Do not claim a task is done until the browser page shows it is done.",
    "Never ask for or persist passwords, tokens, payment card data, or recovery codes.",
    "If the page asks for payment, call pause_for_user with code payment.required.",
    "If human verification blocks progress, call pause_for_user with code human.verification.",
    "Use saved profile facts when they are relevant.",
    "",
    "Available profile facts:",
    factLines,
  ].join("\n");

  return [
    { role: "system", content: system },
    {
      role: "user",
      content: [
        `Instruction: ${request.prompt}`,
        `Review mode: ${request.executionMode}`,
        "Initial page snapshot:",
        JSON.stringify(snapshot).slice(0, MAX_TEXT),
      ].join("\n"),
    },
  ];
}

async function callModel(provider, messages) {
  const baseUrl = String(provider.baseUrl || "").replace(/\/+$/, "");
  const endpoint = baseUrl.endsWith("/chat/completions") ? baseUrl : `${baseUrl}/chat/completions`;
  const response = await fetch(endpoint, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      authorization: `Bearer ${provider.apiKey}`,
    },
    body: JSON.stringify({
      model: provider.model,
      messages,
      tools: toolDefinitions(),
      tool_choice: "auto",
      temperature: 0.2,
    }),
  });

  if (!response.ok) {
    await response.arrayBuffer().catch(() => undefined);
    throw new Error(`AI provider returned ${response.status}.`);
  }

  const payload = await response.json();
  const choice = payload.choices?.[0];
  if (!choice) throw new Error("AI provider returned no choices.");
  return choice;
}

function toolDefinitions() {
  return [
    {
      type: "function",
      function: {
        name: "navigate",
        description: "Open a URL in the current browser tab.",
        parameters: {
          type: "object",
          properties: { url: { type: "string" } },
          required: ["url"],
        },
      },
    },
    {
      type: "function",
      function: {
        name: "click",
        description: "Click a visible element by CSS selector or visible text.",
        parameters: {
          type: "object",
          properties: {
            selector: { type: "string" },
            text: { type: "string" },
          },
        },
      },
    },
    {
      type: "function",
      function: {
        name: "type_text",
        description: "Fill a visible input, textarea, or contenteditable field.",
        parameters: {
          type: "object",
          properties: {
            selector: { type: "string" },
            field: { type: "string" },
            text: { type: "string" },
          },
          required: ["text"],
        },
      },
    },
    {
      type: "function",
      function: {
        name: "wait",
        description: "Wait briefly for navigation, email delivery, or page changes.",
        parameters: {
          type: "object",
          properties: { ms: { type: "number" } },
        },
      },
    },
    {
      type: "function",
      function: {
        name: "snapshot",
        description: "Read the current URL, title, visible text, inputs, buttons, and links.",
        parameters: { type: "object", properties: {} },
      },
    },
    {
      type: "function",
      function: {
        name: "pause_for_user",
        description:
          "Pause when payment, human verification, missing credentials, or user consent is required.",
        parameters: {
          type: "object",
          properties: {
            code: { type: "string" },
            message: { type: "string" },
          },
          required: ["message"],
        },
      },
    },
    {
      type: "function",
      function: {
        name: "done",
        description: "Finish when the user's instruction is complete.",
        parameters: {
          type: "object",
          properties: { message: { type: "string" } },
        },
      },
    },
  ];
}

async function connectToPage(cdpUrl) {
  const deadline = Date.now() + CDP_RETRY_MS;
  let lastError = null;
  while (Date.now() < deadline) {
    try {
      const wsUrl = await resolvePageWebSocket(cdpUrl);
      const client = new CdpClient(wsUrl);
      await client.connect();
      await client.send("Page.enable");
      await client.send("Runtime.enable");
      return client;
    } catch (error) {
      lastError = error;
      await delay(500);
    }
  }
  throw lastError ?? new Error("Could not connect to CDP.");
}

async function resolvePageWebSocket(cdpUrl) {
  const base = normalizeCdpUrl(cdpUrl);
  const list = await getJson(`${base}/json/list`);
  const pageTarget =
    list.find((target) => target.type === "page" && target.webSocketDebuggerUrl) ??
    list.find((target) => target.webSocketDebuggerUrl);
  if (pageTarget?.webSocketDebuggerUrl) {
    return normalizeCdpWebSocketUrl(pageTarget.webSocketDebuggerUrl);
  }

  await fetch(`${base}/json/new?about:blank`, { method: "PUT" }).catch(() => undefined);
  const nextList = await getJson(`${base}/json/list`);
  const nextTarget = nextList.find((target) => target.webSocketDebuggerUrl);
  if (nextTarget?.webSocketDebuggerUrl) {
    return normalizeCdpWebSocketUrl(nextTarget.webSocketDebuggerUrl);
  }
  throw new Error("No debuggable browser page is available.");
}

async function getJson(url) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`CDP endpoint returned ${response.status}.`);
  return response.json();
}

class CdpClient {
  constructor(wsUrl) {
    this.wsUrl = wsUrl;
    this.nextId = 1;
    this.pending = new Map();
    this.socket = null;
  }

  connect() {
    return new Promise((resolve, reject) => {
      const socket = new WebSocket(this.wsUrl);
      this.socket = socket;
      socket.addEventListener("open", () => resolve());
      socket.addEventListener("error", () => reject(new Error("CDP WebSocket error.")));
      socket.addEventListener("close", () => {
        for (const pending of this.pending.values()) {
          pending.reject(new Error("CDP WebSocket closed."));
        }
        this.pending.clear();
      });
      socket.addEventListener("message", (event) => this.onMessage(event.data));
    });
  }

  send(method, params = {}, timeoutMs = 30_000) {
    if (!this.socket || this.socket.readyState !== WebSocket.OPEN) {
      return Promise.reject(new Error("CDP WebSocket is not open."));
    }
    const id = this.nextId;
    this.nextId += 1;
    const payload = { id, method, params };
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`CDP command timed out: ${method}`));
      }, timeoutMs);
      this.pending.set(id, {
        resolve: (value) => {
          clearTimeout(timer);
          resolve(value);
        },
        reject: (error) => {
          clearTimeout(timer);
          reject(error);
        },
      });
      this.socket.send(JSON.stringify(payload));
    });
  }

  async evaluate(expression) {
    const response = await this.send("Runtime.evaluate", {
      expression,
      awaitPromise: true,
      returnByValue: true,
    });
    if (response.exceptionDetails) {
      throw new Error("Page evaluation failed.");
    }
    return response.result?.value;
  }

  onMessage(data) {
    const text = typeof data === "string" ? data : String(data);
    const message = JSON.parse(text);
    if (!message.id) return;
    const pending = this.pending.get(message.id);
    if (!pending) return;
    this.pending.delete(message.id);
    if (message.error) {
      pending.reject(new Error(message.error.message || "CDP command failed."));
    } else {
      pending.resolve(message.result ?? {});
    }
  }

  close() {
    if (this.socket && this.socket.readyState === WebSocket.OPEN) {
      this.socket.close();
    }
  }
}

async function takeSnapshot(page) {
  const raw = await page.evaluate(`(${snapshotFunction.toString()})()`);
  return typeof raw === "string" ? JSON.parse(raw) : raw;
}

function snapshotFunction() {
  const visible = (el) => {
    const style = window.getComputedStyle(el);
    const rect = el.getBoundingClientRect();
    return (
      style.visibility !== "hidden" && style.display !== "none" && rect.width > 0 && rect.height > 0
    );
  };
  const textOf = (el) => (el.innerText || el.textContent || "").replace(/\s+/g, " ").trim();
  const labelFor = (el) => {
    if (el.id) {
      const label = document.querySelector(`label[for="${CSS.escape(el.id)}"]`);
      if (label) return textOf(label);
    }
    const parentLabel = el.closest("label");
    if (parentLabel) return textOf(parentLabel);
    return (
      el.getAttribute("aria-label") ||
      el.getAttribute("name") ||
      el.getAttribute("placeholder") ||
      ""
    );
  };
  const inputs = Array.from(document.querySelectorAll("input, textarea, [contenteditable='true']"))
    .filter(visible)
    .slice(0, 60)
    .map((el, index) => ({
      index,
      tag: el.tagName.toLowerCase(),
      type: el.getAttribute("type") || "",
      name: el.getAttribute("name") || "",
      placeholder: el.getAttribute("placeholder") || "",
      label: labelFor(el),
      value:
        el.getAttribute("type") === "password"
          ? "[password]"
          : String(el.value || el.textContent || "").slice(0, 120),
    }));
  const buttons = Array.from(
    document.querySelectorAll(
      "button, a, [role='button'], input[type='button'], input[type='submit']",
    ),
  )
    .filter(visible)
    .slice(0, 80)
    .map((el, index) => ({
      index,
      tag: el.tagName.toLowerCase(),
      text: textOf(el) || el.getAttribute("value") || el.getAttribute("aria-label") || "",
      href: el.href || "",
    }));
  const links = Array.from(document.querySelectorAll("a[href]"))
    .filter(visible)
    .slice(0, 60)
    .map((el, index) => ({
      index,
      text: textOf(el),
      href: el.href,
    }));
  return JSON.stringify({
    url: location.href,
    title: document.title,
    text: textOf(document.body).slice(0, 6000),
    inputs,
    buttons,
    links,
  });
}

function clickExpression(args) {
  return `(${clickFunction.toString()})(${JSON.stringify({
    selector: stringArg(args.selector),
    text: stringArg(args.text),
  })})`;
}

function clickFunction(args) {
  const visible = (el) => {
    const style = window.getComputedStyle(el);
    const rect = el.getBoundingClientRect();
    return (
      style.visibility !== "hidden" && style.display !== "none" && rect.width > 0 && rect.height > 0
    );
  };
  const textOf = (el) =>
    (el.innerText || el.textContent || el.value || el.getAttribute("aria-label") || "")
      .replace(/\s+/g, " ")
      .trim();
  let target = null;
  if (args.selector) target = document.querySelector(args.selector);
  if (!target && args.text) {
    const needle = args.text.toLowerCase();
    target = Array.from(
      document.querySelectorAll(
        "button, a, [role='button'], input[type='button'], input[type='submit']",
      ),
    )
      .filter(visible)
      .find((el) => textOf(el).toLowerCase().includes(needle));
  }
  if (!target || !visible(target)) return { ok: false };
  target.scrollIntoView({ block: "center", inline: "center" });
  target.click();
  return {
    ok: true,
    label: textOf(target).slice(0, 120) || args.selector || args.text || "element",
  };
}

function typeExpression(args) {
  return `(${typeFunction.toString()})(${JSON.stringify({
    selector: stringArg(args.selector),
    field: stringArg(args.field),
    text: requiredString(args.text, "text"),
  })})`;
}

function typeFunction(args) {
  const visible = (el) => {
    const style = window.getComputedStyle(el);
    const rect = el.getBoundingClientRect();
    return (
      style.visibility !== "hidden" && style.display !== "none" && rect.width > 0 && rect.height > 0
    );
  };
  const labelFor = (el) => {
    if (el.id) {
      const label = document.querySelector(`label[for="${CSS.escape(el.id)}"]`);
      if (label) return label.innerText || label.textContent || "";
    }
    const parentLabel = el.closest("label");
    if (parentLabel) return parentLabel.innerText || parentLabel.textContent || "";
    return "";
  };
  const haystack = (el) =>
    [
      el.getAttribute("name"),
      el.getAttribute("placeholder"),
      el.getAttribute("aria-label"),
      el.getAttribute("autocomplete"),
      labelFor(el),
    ]
      .filter(Boolean)
      .join(" ")
      .toLowerCase();
  let target = null;
  if (args.selector) target = document.querySelector(args.selector);
  if (!target && args.field) {
    const needle = args.field.toLowerCase();
    target = Array.from(document.querySelectorAll("input, textarea, [contenteditable='true']"))
      .filter(visible)
      .find((el) => haystack(el).includes(needle));
  }
  if (!target || !visible(target)) return { ok: false };
  target.scrollIntoView({ block: "center", inline: "center" });
  target.focus();
  if (target.isContentEditable) {
    target.textContent = args.text;
  } else {
    target.value = args.text;
  }
  target.dispatchEvent(
    new InputEvent("input", { bubbles: true, inputType: "insertText", data: args.text }),
  );
  target.dispatchEvent(new Event("change", { bubbles: true }));
  return {
    ok: true,
    label:
      args.field ||
      target.getAttribute("name") ||
      target.getAttribute("placeholder") ||
      args.selector ||
      "field",
  };
}

function classifyPage(snapshot) {
  const text = `${snapshot.title || ""}\n${snapshot.text || ""}`.toLowerCase();
  if (
    /(credit card|card number|payment method|billing address|checkout|stripe|paypal|pay now)/i.test(
      text,
    )
  ) {
    return "payment";
  }
  if (
    /(captcha|recaptcha|hcaptcha|turnstile|verify you are human|are you human|cloudflare|security check)/i.test(
      text,
    )
  ) {
    return "humanVerification";
  }
  return "clear";
}

function emitResult(status, message, code) {
  emit({
    type: "result",
    status,
    message,
    pendingApprovalCode: code,
    pendingApprovalMessage: status === "pausedForUser" ? message : undefined,
  });
}

function emit(message) {
  process.stdout.write(`${JSON.stringify(message)}\n`);
}

function readStdin() {
  return new Promise((resolve, reject) => {
    let data = "";
    process.stdin.setEncoding("utf8");
    process.stdin.on("data", (chunk) => {
      data += chunk;
    });
    process.stdin.on("end", () => resolve(data));
    process.stdin.on("error", reject);
  });
}

function normalizeUrl(value) {
  const raw = String(value).trim();
  const candidate = /^[a-z][a-z0-9+.-]*:/i.test(raw) ? raw : `https://${raw}`;
  let url;
  try {
    url = new URL(candidate);
  } catch {
    throw new Error("Only valid http:// or https:// URLs can be opened.");
  }
  if (url.username || url.password) {
    throw new Error("URLs containing embedded credentials are not allowed.");
  }
  if (url.protocol === "https:") return url.toString();
  if (url.protocol === "http:" && isLocalHost(url.hostname)) return url.toString();
  throw new Error("Only https:// URLs are allowed, except local http:// endpoints.");
}

function normalizeCdpUrl(value) {
  let url;
  try {
    url = new URL(String(value || "").trim());
  } catch {
    throw new Error("Invalid browser automation endpoint.");
  }
  if (url.protocol !== "http:" || !isLocalHost(url.hostname)) {
    throw new Error("Browser automation endpoint must be local.");
  }
  if (
    url.username ||
    url.password ||
    url.search ||
    url.hash ||
    (url.pathname && url.pathname !== "/")
  ) {
    throw new Error("Browser automation endpoint contains unsupported URL parts.");
  }
  if (!/^\d+$/.test(url.port)) {
    throw new Error("Browser automation endpoint must include a local port.");
  }
  return url.toString().replace(/\/+$/, "");
}

function normalizeCdpWebSocketUrl(value) {
  let url;
  try {
    url = new URL(String(value || "").trim());
  } catch {
    throw new Error("Invalid browser automation WebSocket endpoint.");
  }
  if (!["ws:", "wss:"].includes(url.protocol) || !isLocalHost(url.hostname)) {
    throw new Error("Browser automation WebSocket endpoint must be local.");
  }
  if (url.username || url.password || !/^\d+$/.test(url.port)) {
    throw new Error("Browser automation WebSocket endpoint contains unsupported URL parts.");
  }
  return url.toString();
}

function isLocalHost(hostname) {
  const host = String(hostname || "").toLowerCase();
  return host === "localhost" || host === "127.0.0.1" || host === "::1" || host === "[::1]";
}

function parseToolArgs(value) {
  if (!value) return {};
  if (typeof value === "object") return value;
  try {
    return JSON.parse(value);
  } catch {
    return {};
  }
}

function requiredString(value, name) {
  const text = stringArg(value);
  if (!text) throw new Error(`Missing required tool argument: ${name}`);
  return text;
}

function stringArg(value) {
  return typeof value === "string" ? value.trim() : "";
}

function shortMessage(error) {
  return truncate(error instanceof Error ? error.message : String(error), 900);
}

function truncate(value, max) {
  const text = String(value ?? "");
  return text.length > max ? `${text.slice(0, max)}...` : text;
}

function clamp(value, min, max) {
  if (!Number.isFinite(value)) return min;
  return Math.min(Math.max(value, min), max);
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
