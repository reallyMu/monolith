/**
 * Unit tests for RAG endpoint compose/parse.
 *   node --experimental-strip-types scripts/test-rag-engines.mjs
 */
import assert from "node:assert/strict";
import {
  composeRagEndpoint,
  defaultConnParts,
  parseRagEndpoint,
  patchRagStoreEndpoint,
  ragEngineMeta,
} from "../src/utils/ragEngines.ts";

assert.equal(ragEngineMeta("qdrant").defaultPort, 6333);
assert.throws(() => ragEngineMeta("nope"), /unknown RAG engine/);

const httpParts = defaultConnParts("http");
assert.equal(httpParts.host, "127.0.0.1");
assert.equal(httpParts.path, "/search");
const http = composeRagEndpoint("http", httpParts);
assert.equal(http.endpoint, "http://127.0.0.1:8080/search");

const roundHttp = parseRagEndpoint("http", http.endpoint, "tok");
assert.equal(roundHttp.host, "127.0.0.1");
assert.equal(roundHttp.port, "8080");
assert.equal(roundHttp.path, "/search");
assert.equal(roundHttp.password, "tok");

assert.throws(
  () => composeRagEndpoint("http", { ...httpParts, host: "" }),
  /host is empty/,
);
assert.throws(
  () => composeRagEndpoint("http", { ...httpParts, path: "" }),
  /path is empty/,
);

const pg = composeRagEndpoint("pgvector", {
  host: "127.0.0.1",
  port: "5432",
  path: "",
  username: "muqiang",
  password: "s3cret",
  database: "postgres",
});
assert.equal(pg.endpoint, "postgresql://muqiang:s3cret@127.0.0.1:5432/postgres");
const roundPg = parseRagEndpoint("pgvector", pg.endpoint, "");
assert.equal(roundPg.username, "muqiang");
assert.equal(roundPg.password, "s3cret");
assert.equal(roundPg.database, "postgres");

assert.throws(
  () => composeRagEndpoint("pgvector", { ...defaultConnParts("pgvector"), username: "" }),
  /username is empty/,
);

assert.throws(() => parseRagEndpoint("qdrant", "http://only-host", ""), /missing port/);

const patched = patchRagStoreEndpoint("qdrant", "http://127.0.0.1:6333", "k", {
  port: "6334",
});
assert.equal(patched.endpoint, "http://127.0.0.1:6334");
assert.equal(patched.apiKey, "k");

const lance = composeRagEndpoint("lancedb", {
  ...defaultConnParts("lancedb"),
  path: "/tmp/vectors",
});
assert.equal(lance.endpoint, "/tmp/vectors");
assert.throws(
  () => composeRagEndpoint("lancedb", { ...defaultConnParts("lancedb"), path: "" }),
  /path is empty/,
);

console.log("test-rag-engines: ok");
