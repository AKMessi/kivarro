import { invoke } from "@tauri-apps/api/core";
import type {
  ApiStatus,
  BenchmarkResult,
  HardwareSnapshot,
  LogEntry,
  ModelRecord,
  RuntimeMetrics,
} from "./types";

const fallbackHardware: HardwareSnapshot = {
  os: "preview",
  architecture: "browser",
  cpuBrand: "Tauri backend unavailable",
  cpuCores: 0,
  cpuUtilizationPercent: 0,
  ramTotalGib: 0,
  ramUsedGib: 0,
  blocks: [
    {
      id: "preview",
      name: "Preview Runtime",
      kind: "Shell",
      status: "Open with Tauri for local telemetry",
      utilizationPercent: 0,
      memoryTotalGib: null,
      memoryUsedGib: null,
      segments: [],
    },
  ],
};

const fallbackMetrics: RuntimeMetrics = {
  activeModel: "No model loaded",
  activeBackend: "Preview shell",
  serverUrl: "http://127.0.0.1:8080/v1",
  apiPort: 8080,
  apiOnline: false,
  tokensPerSecond: 0,
  contextUsedTokens: 0,
  contextTotalTokens: 32768,
  cpuUtilizationPercent: 0,
  gpuUtilizationPercent: 0,
  ramUsedGib: 0,
  ramTotalGib: 0,
};

const fallbackApiStatus: ApiStatus = {
  enabled: false,
  port: 8080,
  baseUrl: "http://127.0.0.1:8080/v1",
  endpoints: [
    {
      method: "POST",
      path: "/v1/chat/completions",
      description: "OpenAI-compatible streaming chat completions",
      status: "Planned",
    },
    {
      method: "POST",
      path: "/v1/embeddings",
      description: "Local embedding generation for RAG pipelines",
      status: "Planned",
    },
  ],
};

async function safeInvoke<T>(command: string, fallback: T): Promise<T> {
  if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) {
    return fallback;
  }

  try {
    return await invoke<T>(command);
  } catch (error) {
    console.warn(`Kivarro command failed: ${command}`, error);
    return fallback;
  }
}

export function getHardwareSnapshot(): Promise<HardwareSnapshot> {
  return safeInvoke("get_hardware_snapshot", fallbackHardware);
}

export function getRuntimeMetrics(): Promise<RuntimeMetrics> {
  return safeInvoke("get_runtime_metrics", fallbackMetrics);
}

export function listModels(): Promise<ModelRecord[]> {
  return safeInvoke("list_models", []);
}

export function getApiStatus(): Promise<ApiStatus> {
  return safeInvoke("get_api_status", fallbackApiStatus);
}

export function listBenchmarkResults(): Promise<BenchmarkResult[]> {
  return safeInvoke("list_benchmark_results", []);
}

export function listSystemLogs(): Promise<LogEntry[]> {
  return safeInvoke("list_system_logs", [
    {
      level: "INFO",
      source: "preview",
      message: "Open Kivarro in Tauri to connect local telemetry.",
      timestamp: "preview",
    },
  ]);
}
