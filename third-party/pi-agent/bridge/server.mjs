/**
 * Monolith ↔ pi-coding-agent HTTP bridge.
 * Health + SSE prompt for the in-app Agent float.
 *
 * Env:
 *   PI_BRIDGE_PORT   (default 8096)
 *   PI_AGENT_WORKDIR agent cwd / tools workspace
 *   PI_AGENT_DIR     agent config dir (.pi-agent equivalent)
 */
import { createServer } from "node:http";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { createAgentSession } from "@earendil-works/pi-coding-agent";

const PORT = Number(process.env.PI_BRIDGE_PORT || 8096);
const WORKDIR = process.env.PI_AGENT_WORKDIR || process.cwd();
const BRIDGE_ROOT = fileURLToPath(new URL(".", import.meta.url));
const APP_ROOT = join(BRIDGE_ROOT, "..");
const AGENT_DIR = process.env.PI_AGENT_DIR || join(APP_ROOT, ".pi-agent");

let sessionHandle = null;
let sessionInitError = null;
/** @type {{ session: { abort?: () => Promise<void> } } | null} */
let activeRun = null;

async function initSession() {
  if (sessionHandle) return sessionHandle;
  if (sessionInitError) throw sessionInitError;
  try {
    process.chdir(WORKDIR);
    const { session } = await createAgentSession({
      cwd: WORKDIR,
      agentDir: AGENT_DIR,
      thinkingLevel: "off",
    });
    sessionHandle = session;
    const mid = `${session.model?.provider ?? "?"}/${session.model?.id ?? "?"}`;
    console.log(
      `[monolith-pi-bridge] session ready model=${mid} thinking=${session.thinkingLevel} agentDir=${AGENT_DIR}`,
    );
    return session;
  } catch (err) {
    sessionInitError = err;
    throw err;
  }
}

function readJsonBody(req) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    req.on("data", (c) => chunks.push(c));
    req.on("end", () => {
      const raw = Buffer.concat(chunks).toString("utf8").trim();
      if (!raw) {
        resolve({});
        return;
      }
      try {
        resolve(JSON.parse(raw));
      } catch (e) {
        reject(e);
      }
    });
    req.on("error", reject);
  });
}

function sendJson(res, status, obj) {
  const body = JSON.stringify(obj);
  res.writeHead(status, {
    "Content-Type": "application/json; charset=utf-8",
    "Content-Length": Buffer.byteLength(body),
  });
  res.end(body);
}

function healthPayload() {
  return {
    ok: true,
    service: "monolith-pi-bridge",
    port: PORT,
    workdir: WORKDIR,
    agentDir: AGENT_DIR,
    sessionReady: sessionHandle != null,
    model: sessionHandle
      ? `${sessionHandle.model?.provider ?? ""}/${sessionHandle.model?.id ?? ""}`
      : null,
    thinkingLevel: sessionHandle?.thinkingLevel ?? null,
    initError: sessionInitError ? String(sessionInitError) : null,
  };
}

async function handleHealth(res) {
  sendJson(res, 200, healthPayload());
}

async function handleAbort(res) {
  if (activeRun?.session && typeof activeRun.session.abort === "function") {
    try {
      await activeRun.session.abort();
    } catch (err) {
      console.warn("[monolith-pi-bridge] abort failed", err);
    }
  }
  sendJson(res, 200, { ok: true, hadActiveRun: activeRun != null });
}

async function handlePrompt(req, res) {
  const body = await readJsonBody(req);
  const message = typeof body.message === "string" ? body.message.trim() : "";
  if (!message) {
    sendJson(res, 400, { error: "message must not be empty" });
    return;
  }

  res.writeHead(200, {
    "Content-Type": "text/event-stream; charset=utf-8",
    "Cache-Control": "no-cache",
    Connection: "keep-alive",
  });
  res.write(": connected\n\n");

  const writeEvent = (event, data) => {
    if (res.writableEnded) return;
    res.write(`event: ${event}\n`);
    res.write(`data: ${JSON.stringify(data)}\n\n`);
  };

  let session;
  try {
    session = await initSession();
  } catch (err) {
    writeEvent("error", { error: String(err) });
    res.end();
    return;
  }

  let fullText = "";
  let thinkingText = "";
  let clientClosed = false;
  let aborted = false;

  const unsubscribe = session.subscribe((event) => {
    if (event.type !== "message_update") return;
    const ame = event.assistantMessageEvent;
    if (!ame) return;
    if (ame.type === "text_delta") {
      const delta = ame.delta || "";
      fullText += delta;
      writeEvent("delta", { delta });
      return;
    }
    if (ame.type === "thinking_delta") {
      thinkingText += ame.delta || "";
    }
  });

  activeRun = { session };

  req.on("close", () => {
    clientClosed = true;
    if (typeof session.abort === "function") {
      void session.abort().catch((err) => {
        console.warn("[monolith-pi-bridge] client close abort failed", err);
      });
    }
  });

  try {
    await session.prompt(message);
    if (!fullText.trim() && thinkingText.trim()) {
      fullText = thinkingText.trim();
      writeEvent("delta", { delta: fullText });
      console.warn(
        "[monolith-pi-bridge] empty content; fell back to thinking text",
      );
    }
    if (!clientClosed && !aborted) {
      writeEvent("done", {
        text: fullText,
        model: `${session.model?.provider ?? ""}/${session.model?.id ?? ""}`,
      });
    }
  } catch (err) {
    const errText = String(err);
    aborted =
      clientClosed ||
      errText.toLowerCase().includes("abort") ||
      errText.toLowerCase().includes("cancel");
    if (aborted) {
      writeEvent("done", { text: fullText, aborted: true });
    } else if (!clientClosed) {
      writeEvent("error", { error: errText });
    }
  } finally {
    unsubscribe();
    activeRun = null;
    if (!res.writableEnded) res.end();
  }
}

const server = createServer(async (req, res) => {
  const url = req.url?.split("?")[0] || "/";

  if (req.method === "GET" && (url === "/health" || url === "/api/health")) {
    await handleHealth(res);
    return;
  }

  if (req.method === "POST" && url === "/api/prompt") {
    try {
      await handlePrompt(req, res);
    } catch (err) {
      if (!res.headersSent) {
        sendJson(res, 500, { error: String(err) });
      }
    }
    return;
  }

  if (req.method === "POST" && url === "/api/abort") {
    try {
      await handleAbort(res);
    } catch (err) {
      sendJson(res, 500, { error: String(err) });
    }
    return;
  }

  sendJson(res, 404, { error: "not found" });
});

process.on("uncaughtException", (err) => {
  console.error("[monolith-pi-bridge] uncaughtException", err);
});
process.on("unhandledRejection", (err) => {
  console.error("[monolith-pi-bridge] unhandledRejection", err);
});

server.listen(PORT, "127.0.0.1", () => {
  console.log(
    `[monolith-pi-bridge] listening http://127.0.0.1:${PORT} workdir=${WORKDIR} agentDir=${AGENT_DIR}`,
  );
});
