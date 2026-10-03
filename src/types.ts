export type ViewMode = "edit" | "preview" | "split";

export interface EditorTab {
  id: string;
  path: string | null;
  title: string;
  content: string;
  dirty: boolean;
  language: string;
  viewMode: ViewMode;
  cursorLine: number;
  cursorCol: number;
}
