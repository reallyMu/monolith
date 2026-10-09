/**
 * RAG store engines. Most server DBs = host + port (+ optional auth).
 * Exceptions: PostgreSQL needs user/db; LanceDB is often a local path; HTTP Search needs a URL path.
 */

export type RagEngineId =
  | "http"
  | "pgvector"
  | "qdrant"
  | "milvus"
  | "chroma"
  | "weaviate"
  | "lancedb"
  | "elasticsearch";

export type RagFormMode = "hostport" | "path";

export type RagEngineMeta = {
  id: RagEngineId;
  label: string;
  formMode: RagFormMode;
  defaultHost: string;
  defaultPort: number;
  /** HTTP Search path (e.g. /search) or unused. */
  defaultPath: string;
  defaultQueryMode: "text" | "vector";
  usesApiKey: boolean;
  /** PostgreSQL: username + database (+ password in apiKey). */
  usesPgAuth: boolean;
  blurb: string;
};

export type RagConnParts = {
  host: string;
  port: string;
  path: string;
  username: string;
  password: string;
  database: string;
};

export const RAG_ENGINES: RagEngineMeta[] = [
  {
    id: "http",
    label: "自定义 HTTP 检索",
    formMode: "hostport",
    defaultHost: "127.0.0.1",
    defaultPort: 8080,
    defaultPath: "/search",
    defaultQueryMode: "text",
    usesApiKey: true,
    usesPgAuth: false,
    blurb: "自研/托管 Search API（非具名向量库）：主机、端口、路径如 /search",
  },
  {
    id: "pgvector",
    label: "PostgreSQL (pgvector)",
    formMode: "hostport",
    defaultHost: "127.0.0.1",
    defaultPort: 5432,
    defaultPath: "",
    defaultQueryMode: "vector",
    usesApiKey: false,
    usesPgAuth: true,
    blurb: "主机、端口，外加用户名、密码、数据库名",
  },
  {
    id: "qdrant",
    label: "Qdrant",
    formMode: "hostport",
    defaultHost: "127.0.0.1",
    defaultPort: 6333,
    defaultPath: "",
    defaultQueryMode: "vector",
    usesApiKey: true,
    usesPgAuth: false,
    blurb: "主机 + 端口；有鉴权时填 API Key",
  },
  {
    id: "milvus",
    label: "Milvus",
    formMode: "hostport",
    defaultHost: "127.0.0.1",
    defaultPort: 19530,
    defaultPath: "",
    defaultQueryMode: "vector",
    usesApiKey: true,
    usesPgAuth: false,
    blurb: "主机 + 端口；有鉴权时填 API Key",
  },
  {
    id: "chroma",
    label: "Chroma",
    formMode: "hostport",
    defaultHost: "127.0.0.1",
    defaultPort: 8000,
    defaultPath: "",
    defaultQueryMode: "vector",
    usesApiKey: true,
    usesPgAuth: false,
    blurb: "主机 + 端口；有鉴权时填 API Key",
  },
  {
    id: "weaviate",
    label: "Weaviate",
    formMode: "hostport",
    defaultHost: "127.0.0.1",
    defaultPort: 8080,
    defaultPath: "",
    defaultQueryMode: "vector",
    usesApiKey: true,
    usesPgAuth: false,
    blurb: "主机 + 端口；有鉴权时填 API Key",
  },
  {
    id: "elasticsearch",
    label: "Elasticsearch",
    formMode: "hostport",
    defaultHost: "127.0.0.1",
    defaultPort: 9200,
    defaultPath: "",
    defaultQueryMode: "vector",
    usesApiKey: true,
    usesPgAuth: false,
    blurb: "主机 + 端口；有鉴权时填 API Key",
  },
  {
    id: "lancedb",
    label: "LanceDB",
    formMode: "path",
    defaultHost: "",
    defaultPort: 0,
    defaultPath: "",
    defaultQueryMode: "vector",
    usesApiKey: false,
    usesPgAuth: false,
    blurb: "本地嵌入式库：填磁盘目录路径（不是 IP）",
  },
];

export function ragEngineMeta(kind: string): RagEngineMeta {
  const found = RAG_ENGINES.find((e) => e.id === kind);
  if (!found) {
    throw new Error(`unknown RAG engine kind: ${kind || "(empty)"}`);
  }
  return found;
}

/** Form defaults when adding a new RAG store row (not used to invent missing runtime values). */
export function defaultConnParts(kind: string): RagConnParts {
  const m = ragEngineMeta(kind);
  return {
    host: m.defaultHost,
    port: m.defaultPort > 0 ? String(m.defaultPort) : "",
    path: m.defaultPath,
    username: m.usesPgAuth ? "postgres" : "",
    password: "",
    database: m.usesPgAuth ? "postgres" : "",
  };
}

/** Build the stored `endpoint` (+ password/apiKey) from form parts. */
export function composeRagEndpoint(
  kind: string,
  parts: RagConnParts,
): { endpoint: string; apiKey: string } {
  const m = ragEngineMeta(kind);
  if (m.formMode === "path") {
    const path = parts.path.trim();
    if (!path) throw new Error("LanceDB path is empty");
    return { endpoint: path, apiKey: parts.password };
  }
  const host = parts.host.trim().replace(/\/$/, "");
  const port = parts.port.trim().replace(/\D/g, "");
  if (!host) throw new Error("RAG host is empty");
  if (!port) throw new Error("RAG port is empty");
  if (m.usesPgAuth) {
    const userRaw = parts.username.trim();
    const dbRaw = parts.database.trim();
    if (!userRaw) throw new Error("PostgreSQL username is empty");
    if (!dbRaw) throw new Error("PostgreSQL database is empty");
    const user = encodeURIComponent(userRaw);
    const pass = parts.password ? `:${encodeURIComponent(parts.password)}` : "";
    const db = encodeURIComponent(dbRaw);
    return {
      endpoint: `postgresql://${user}${pass}@${host}:${port}/${db}`,
      apiKey: "",
    };
  }
  let path = parts.path.trim();
  if (kind === "http") {
    if (!path) throw new Error("HTTP Search path is empty");
    if (!path.startsWith("/")) path = `/${path}`;
  } else {
    path = "";
  }
  return {
    endpoint: `http://${host}:${port}${path}`,
    apiKey: parts.password,
  };
}

/** Parse stored endpoint back into form parts. Empty endpoint → form defaults for that kind. */
export function parseRagEndpoint(
  kind: string,
  endpoint: string,
  apiKey: string,
): RagConnParts {
  const base = defaultConnParts(kind);
  const ep = (endpoint || "").trim();
  const m = ragEngineMeta(kind);
  if (!ep) {
    return { ...base, password: apiKey };
  }
  if (m.formMode === "path") {
    return { ...base, path: ep, password: apiKey };
  }
  if (m.usesPgAuth) {
    const u = new URL(ep.replace(/^postgresql:/i, "http:"));
    if (!u.hostname) throw new Error(`invalid PostgreSQL endpoint: ${ep}`);
    if (!u.port) throw new Error(`PostgreSQL endpoint missing port: ${ep}`);
    const db = decodeURIComponent((u.pathname || "").replace(/^\//, ""));
    if (!db) throw new Error(`PostgreSQL endpoint missing database: ${ep}`);
    if (!u.username) throw new Error(`PostgreSQL endpoint missing username: ${ep}`);
    return {
      host: u.hostname,
      port: u.port,
      path: "",
      username: decodeURIComponent(u.username),
      password: decodeURIComponent(u.password || ""),
      database: db,
    };
  }
  const u = new URL(ep.includes("://") ? ep : `http://${ep}`);
  if (!u.hostname) throw new Error(`invalid RAG endpoint: ${ep}`);
  if (!u.port) throw new Error(`RAG endpoint missing port: ${ep}`);
  return {
    host: u.hostname,
    port: u.port,
    path: u.pathname && u.pathname !== "/" ? u.pathname : kind === "http" ? "" : "",
    username: "",
    password: apiKey,
    database: "",
  };
}

export function patchRagStoreEndpoint(
  kind: string,
  endpoint: string,
  apiKey: string,
  patch: Partial<RagConnParts>,
): { endpoint: string; apiKey: string } {
  const parts = { ...parseRagEndpoint(kind, endpoint, apiKey), ...patch };
  return composeRagEndpoint(kind, parts);
}
