export type ViewId =
  | "command"
  | "models"
  | "hardware"
  | "tuning"
  | "knowledge"
  | "agents"
  | "api"
  | "benchmarks"
  | "logs"
  | "settings";

export interface MemorySegment {
  label: string;
  gib: number;
  color: string;
}

export interface ComputeBlock {
  id: string;
  name: string;
  kind: string;
  status: string;
  utilizationPercent: number;
  memoryTotalGib?: number | null;
  memoryUsedGib?: number | null;
  segments: MemorySegment[];
}

export interface HardwareSnapshot {
  os: string;
  architecture: string;
  cpuBrand: string;
  cpuCores: number;
  cpuUtilizationPercent: number;
  ramTotalGib: number;
  ramUsedGib: number;
  blocks: ComputeBlock[];
}

export interface RuntimeMetrics {
  activeModel: string;
  activeBackend: string;
  serverUrl: string;
  apiPort: number;
  apiOnline: boolean;
  tokensPerSecond: number;
  contextUsedTokens: number;
  contextTotalTokens: number;
  cpuUtilizationPercent: number;
  gpuUtilizationPercent: number;
  ramUsedGib: number;
  ramTotalGib: number;
}

export interface ModelRecord {
  id: string;
  name: string;
  path: string;
  format: string;
  sizeGib: number;
  status: string;
  fit: string;
}

export interface ApiEndpoint {
  method: string;
  path: string;
  description: string;
  status: string;
}

export interface ApiStatus {
  enabled: boolean;
  port: number;
  baseUrl: string;
  endpoints: ApiEndpoint[];
}

export interface BenchmarkResult {
  model: string;
  backend: string;
  evalCount: number;
  evalDurationMs: number;
  tokensPerSecond: number;
  loadDurationMs: number;
}

export interface LogEntry {
  level: "INFO" | "WARN" | "ERROR" | "DEBUG" | string;
  source: string;
  message: string;
  timestamp: string;
}
