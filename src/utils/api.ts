import { invoke } from "@tauri-apps/api/core";

export function readTextFile(path: string): Promise<string> {
  return invoke<string>("read_text_file", { path });
}

export function writeTextFile(path: string, content: string): Promise<void> {
  return invoke<void>("write_text_file", { path, content });
}

export function listRecent(): Promise<string[]> {
  return invoke<string[]>("list_recent");
}

export function pushRecent(path: string): Promise<string[]> {
  return invoke<string[]>("push_recent", { path });
}

export function clearRecent(): Promise<void> {
  return invoke<void>("clear_recent");
}

/** Paths from Finder “Open With” / argv, queued before the UI was ready. */
export function takePendingOpens(): Promise<string[]> {
  return invoke<string[]>("take_pending_opens");
}

/** Open path with the OS default application. */
export function openInOs(path: string): Promise<void> {
  return invoke<void>("open_in_os", { path });
}
