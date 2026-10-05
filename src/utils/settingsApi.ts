import { invoke } from "@tauri-apps/api/core";

export interface AppSettings {
  inboxDir: string;
  defaultOpenDir: string;
  defaultSaveDir: string;
}

export interface SettingsView {
  settings: AppSettings;
  assetsDbPath: string;
  appDataDir: string;
  mcpBinaryPath: string;
  clipperReleaseDir: string;
}

export function settingsGet(): Promise<SettingsView> {
  return invoke("settings_get");
}

export function settingsSave(settings: AppSettings): Promise<SettingsView> {
  return invoke("settings_save", { settings });
}
