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
  architecture?: string | null;
  parameterSize?: string | null;
  quantization?: string | null;
  contextLength?: number | null;
  blockCount?: number | null;
  tensorCount?: number | null;
  ggufVersion?: number | null;
  metadataSource: string;
}

export interface SamplingParameters {
  temperature: number;
  topP: number;
  topK: number;
  minP: number;
  typicalP: number;
  repeatPenalty: number;
  repeatLastN: number;
  presencePenalty: number;
  frequencyPenalty: number;
  mirostatMode: number;
  mirostatTau: number;
  mirostatEta: number;
  seed: number | null;
  maxTokens: number;
  stopSequences: string[];
}

export interface RuntimeParameters {
  backend: string;
  contextLength: number;
  batchSize: number;
  microBatchSize: number;
  cpuThreads: number;
  gpuLayers: number;
  tensorSplit: number[];
  mainGpu: number | null;
  useMmap: boolean;
  useMlock: boolean;
  flashAttention: boolean;
  kvCacheQuantization: string;
  ropeFrequencyBase: number | null;
  ropeFrequencyScale: number | null;
}

export interface LogitBiasEntry {
  token: string;
  bias: number;
}

export interface OutputConstraints {
  mode: string;
  jsonSchema: string;
  grammar: string;
  logitBias: LogitBiasEntry[];
  logprobs: boolean;
  topLogprobs: number;
}

export interface InferenceProfile {
  id: string;
  name: string;
  description: string;
  systemPrompt: string;
  sampling: SamplingParameters;
  runtime: RuntimeParameters;
  output: OutputConstraints;
  createdAt: string;
  updatedAt: string;
}

export interface LoadPlanSegment {
  label: string;
  gib: number;
  color: string;
}

export interface ModelLoadPlan {
  modelId: string;
  profileId: string;
  backend: string;
  fit: string;
  recommendation: string;
  modelName: string;
  architecture?: string | null;
  parameterSize?: string | null;
  quantization?: string | null;
  modelContextLength?: number | null;
  metadataSource: string;
  estimatedLayers: number;
  gpuLayers: number;
  cpuLayers: number;
  modelWeightsGib: number;
  kvCacheGib: number;
  runtimeOverheadGib: number;
  totalRequiredGib: number;
  ramTotalGib: number;
  ramAvailableGib: number;
  segments: LoadPlanSegment[];
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

export interface EngineStatus {
  backend: string;
  state: string;
  message: string;
  configured: boolean;
  binaryPath?: string | null;
  pid?: number | null;
  activeModelId?: string | null;
  activeModelName?: string | null;
  host: string;
  port: number;
  baseUrl: string;
  healthOk: boolean;
  lastTokensPerSecond: number;
  contextUsedTokens: number;
  contextTotalTokens: number;
}

export interface ChatTurn {
  role: "system" | "user" | "assistant";
  content: string;
}

export interface InferenceRunResult {
  content: string;
  model: string;
  backend: string;
  elapsedMs: number;
  tokensPerSecond: number;
  promptTokens?: number | null;
  completionTokens?: number | null;
  totalTokens?: number | null;
  finishReason?: string | null;
}

export interface InferenceStreamEvent {
  requestId: string;
  phase: "started" | "delta" | "done" | "error" | "cancelled" | string;
  delta: string;
  content: string;
  model: string;
  completionTokens: number;
  tokensPerSecond: number;
  elapsedMs: number;
  finishReason?: string | null;
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
