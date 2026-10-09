import { invoke } from "@tauri-apps/api/core";

export interface LlmEndpoint {
  id: string;
  name: string;
  baseUrl: string;
  apiKey: string;
  model: string;
  role: string;
  enabled: boolean;
}

export interface RagStore {
  id: string;
  name: string;
  kind: string;
  endpoint: string;
  apiKey: string;
  queryMode: string;
  enabled: boolean;
  topK: number;
}

export interface AgentMcpServer {
  id: string;
  name: string;
  command: string;
  args: string[];
  enabled: boolean;
}

export interface AgentSkillEntry {
  id: string;
  name: string;
  source: string;
  path: string;
  remoteUrl: string;
  enabled: boolean;
}

export interface AppSettings {
  inboxDir: string;
  /** Agent/MCP default folder for newly created asset files. */
  assetsDir: string;
  defaultOpenDir: string;
  defaultSaveDir: string;
  llms: LlmEndpoint[];
  activeChatLlmId: string;
  activeEmbedLlmId: string;
  ragStores: RagStore[];
  agentMcpServers: AgentMcpServer[];
  agentSkills: AgentSkillEntry[];
  agentBridgePort: number;
}

export interface SettingsView {
  settings: AppSettings;
  assetsDbPath: string;
  appDataDir: string;
  mcpBinaryPath: string;
  clipperReleaseDir: string;
  skillsDir: string;
}

export function emptySettings(): AppSettings {
  return {
    inboxDir: "",
    assetsDir: "",
    defaultOpenDir: "",
    defaultSaveDir: "",
    llms: [],
    activeChatLlmId: "",
    activeEmbedLlmId: "",
    ragStores: [],
    agentMcpServers: [],
    agentSkills: [],
    agentBridgePort: 8096,
  };
}

export function settingsGet(): Promise<SettingsView> {
  return invoke("settings_get");
}

export function settingsSave(settings: AppSettings): Promise<SettingsView> {
  return invoke("settings_save", { settings });
}
