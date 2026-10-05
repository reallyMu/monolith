import { invoke } from "@tauri-apps/api/core";

export interface InboxEntry {
  path: string;
  name: string;
  registered: boolean;
  assetId: number | null;
}

export function clipperOpenInstall(): Promise<string> {
  return invoke("clipper_open_install");
}

export function clipperListInbox(): Promise<InboxEntry[]> {
  return invoke("clipper_list_inbox");
}

export interface DiscoveredAgent {
  id: string;
  name: string;
  mcpConfigPath: string | null;
  skillDirs: string[];
  installable: boolean;
  alreadyHasMonolith: boolean;
  skipReason: string | null;
}

export function mcpDiscoverAgents(): Promise<DiscoveredAgent[]> {
  return invoke("mcp_discover_agents");
}

export function mcpInstallForAgents(agents: string[]): Promise<{
  binaryPath: string;
  notes: string[];
}> {
  return invoke("mcp_install_for_agents", { agents });
}

export interface McpSkillView {
  path: string;
  content: string;
  reloadTargets: string[];
}

export function mcpSkillGet(): Promise<McpSkillView> {
  return invoke("mcp_skill_get");
}

export function mcpSkillReload(agents: string[] = []): Promise<{ notes: string[] }> {
  return invoke("mcp_skill_reload", { agents });
}
