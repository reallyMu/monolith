/**
 * Unit tests for Pi Bridge chat helpers (no Tauri).
 *   node --experimental-strip-types scripts/test-agent-api.mjs
 */
import assert from "node:assert/strict";
import {
  dispatchPiSseBlock,
  llmBase,
  llmHeaders,
  requireBridgeBaseUrl,
  requireChatModel,
} from "../src/utils/agentApi.ts";

const llm = (over = {}) => ({
  id: "llm-1",
  name: "t",
  baseUrl: "http://127.0.0.1:8000/v1/",
  apiKey: "k",
  model: "m1",
  role: "chat",
  enabled: true,
  ...over,
});

assert.equal(llmBase(llm()), "http://127.0.0.1:8000/v1");
assert.throws(() => llmBase(llm({ baseUrl: "  " })), /base URL empty/);

assert.equal(requireChatModel(llm()), "m1");
assert.throws(() => requireChatModel(llm({ model: "" })), /empty model/);

assert.deepEqual(llmHeaders(llm()), {
  "Content-Type": "application/json",
  Authorization: "Bearer k",
});
assert.deepEqual(llmHeaders(llm({ apiKey: "" })), {
  "Content-Type": "application/json",
});
assert.deepEqual(llmHeaders(llm({ apiKey: "  " })), {
  "Content-Type": "application/json",
});

assert.equal(requireBridgeBaseUrl("http://127.0.0.1:8096/"), "http://127.0.0.1:8096");
assert.throws(() => requireBridgeBaseUrl(""), /bridge URL empty/);
assert.throws(() => requireBridgeBaseUrl("   "), /bridge URL empty/);

{
  const deltas = [];
  const r = dispatchPiSseBlock(
    'event: delta\ndata: {"delta":"Hi"}',
    "",
    {
      onDelta: (d) => deltas.push(d),
      onDone: () => assert.fail("done"),
      onError: () => assert.fail("error"),
    },
  );
  assert.equal(r.streamedText, "Hi");
  assert.equal(r.terminal, null);
  assert.deepEqual(deltas, ["Hi"]);
}

{
  let doneText = null;
  let meta = null;
  const r = dispatchPiSseBlock(
    'event: done\ndata: {"text":"final","aborted":true}',
    "partial",
    {
      onDelta: () => assert.fail("delta"),
      onDone: (t, m) => {
        doneText = t;
        meta = m;
      },
      onError: () => assert.fail("error"),
    },
  );
  assert.equal(doneText, "final");
  assert.deepEqual(meta, { aborted: true });
  assert.equal(r.terminal, "done");
}

{
  let doneText = null;
  dispatchPiSseBlock(
    'event: done\ndata: {"aborted":false}',
    "from-deltas",
    {
      onDelta: () => {},
      onDone: (t) => {
        doneText = t;
      },
      onError: () => assert.fail("error"),
    },
  );
  assert.equal(doneText, "from-deltas");
}

{
  let err = null;
  const r = dispatchPiSseBlock(
    'event: error\ndata: {"error":"boom"}',
    "",
    {
      onDelta: () => {},
      onDone: () => assert.fail("done"),
      onError: (e) => {
        err = e;
      },
    },
  );
  assert.equal(err, "boom");
  assert.equal(r.terminal, "error");
}

{
  let err = null;
  dispatchPiSseBlock(
    'event: error\ndata: {}',
    "",
    {
      onDelta: () => {},
      onDone: () => assert.fail("done"),
      onError: (e) => {
        err = e;
      },
    },
  );
  assert.match(err, /missing error field/);
}

{
  let err = null;
  dispatchPiSseBlock(
    "event: done\ndata: not-json",
    "",
    {
      onDelta: () => {},
      onDone: () => assert.fail("done"),
      onError: (e) => {
        err = e;
      },
    },
  );
  assert.equal(err, "SSE parse failed");
}

console.log("test-agent-api: ok");
