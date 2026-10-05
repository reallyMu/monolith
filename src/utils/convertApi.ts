import { invoke } from "@tauri-apps/api/core";

export type ConvertResultDto = {
  outputPath: string;
  tool: string;
  status: string;
  logId: number;
};

export type ConversionSourceDto = {
  path: string;
  recordedMtime: number | null;
  recordedSize: number | null;
  currentMtime: number | null;
  currentSize: number | null;
  exists: boolean;
  stale: boolean;
};

export function convertIsSupported(path: string): Promise<boolean> {
  return invoke("convert_is_supported", { path });
}

export function convertToolForPath(path: string): Promise<string> {
  return invoke("convert_tool_for_path", { path });
}

export function convertDefaultOutput(path: string): Promise<string> {
  return invoke("convert_default_output", { path });
}

export function conversionLatestSource(
  outputPath: string,
): Promise<ConversionSourceDto | null> {
  return invoke("conversion_latest_source", { outputPath });
}

export function convertRun(inputPath: string, outputPath: string): Promise<ConvertResultDto> {
  return invoke("convert_run", { inputPath, outputPath });
}

export function convertCancel(): Promise<void> {
  return invoke("convert_cancel");
}
