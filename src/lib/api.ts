import { invoke } from "@tauri-apps/api/core";
import type {
  ApiSettings,
  ApiStatus,
  BenchmarkResult,
  ChatTurn,
  EngineStatus,
  HardwareSnapshot,
  InferenceProfile,
  InferenceRunResult,
  KnowledgeBase,
  KnowledgeBaseDetail,
  KnowledgeDocument,
  LogEntry,
  ModelRecord,
  ModelLoadPlan,
  RetrievalMatch,
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

const fallbackApiSettings: ApiSettings = {
  host: "127.0.0.1",
  port: 8080,
};

const fallbackEngineStatus: EngineStatus = {
  backend: "llama.cpp",
  state: "preview",
  message: "Open Kivarro in Tauri to supervise a local inference engine.",
  configured: false,
  binaryPath: null,
  pid: null,
  activeModelId: null,
  activeModelName: null,
  host: "127.0.0.1",
  port: 8080,
  baseUrl: "http://127.0.0.1:8080/v1",
  healthOk: false,
  lastTokensPerSecond: 0,
  contextUsedTokens: 0,
  contextTotalTokens: 32768,
};

const fallbackKnowledgeBases: KnowledgeBase[] = [
  {
    id: "research-vault",
    name: "Research Vault",
    documentCount: 0,
    chunkCount: 0,
    updatedAt: "preview",
  },
];

export const fallbackProfiles: InferenceProfile[] = [
  {
    id: "balanced-engineer",
    name: "Balanced Engineer",
    description: "General technical work with stable sampling and long-context defaults.",
    systemPrompt:
      "You are a precise local AI assistant. Prefer concise, verifiable answers and surface uncertainty explicitly.",
    sampling: {
      temperature: 0.7,
      topP: 0.92,
      topK: 40,
      minP: 0.05,
      typicalP: 1,
      repeatPenalty: 1.1,
      repeatLastN: 256,
      presencePenalty: 0,
      frequencyPenalty: 0,
      mirostatMode: 0,
      mirostatTau: 5,
      mirostatEta: 0.1,
      seed: null,
      maxTokens: 2048,
      stopSequences: [],
    },
    runtime: {
      backend: "llama.cpp",
      contextLength: 32768,
      batchSize: 512,
      microBatchSize: 128,
      cpuThreads: 4,
      gpuLayers: 0,
      tensorSplit: [],
      mainGpu: null,
      useMmap: true,
      useMlock: false,
      flashAttention: true,
      kvCacheQuantization: "f16",
      ropeFrequencyBase: null,
      ropeFrequencyScale: null,
    },
    output: {
      mode: "text",
      jsonSchema: "",
      grammar: "",
      logitBias: [],
      logprobs: false,
      topLogprobs: 0,
    },
    createdAt: "preview",
    updatedAt: "preview",
  },
  {
    id: "strict-json-extractor",
    name: "Strict JSON Extractor",
    description: "Low-temperature extraction profile with JSON schema constraints ready.",
    systemPrompt: "Return only valid JSON that satisfies the active schema. Do not include prose.",
    sampling: {
      temperature: 0.1,
      topP: 0.85,
      topK: 20,
      minP: 0.01,
      typicalP: 1,
      repeatPenalty: 1.05,
      repeatLastN: 128,
      presencePenalty: 0,
      frequencyPenalty: 0,
      mirostatMode: 0,
      mirostatTau: 5,
      mirostatEta: 0.1,
      seed: 42,
      maxTokens: 2048,
      stopSequences: [],
    },
    runtime: {
      backend: "llama.cpp",
      contextLength: 16384,
      batchSize: 512,
      microBatchSize: 128,
      cpuThreads: 4,
      gpuLayers: 0,
      tensorSplit: [],
      mainGpu: null,
      useMmap: true,
      useMlock: false,
      flashAttention: true,
      kvCacheQuantization: "f16",
      ropeFrequencyBase: null,
      ropeFrequencyScale: null,
    },
    output: {
      mode: "json_schema",
      jsonSchema:
        '{\n  "type": "object",\n  "properties": {},\n  "additionalProperties": true\n}',
      grammar: "",
      logitBias: [],
      logprobs: false,
      topLogprobs: 0,
    },
    createdAt: "preview",
    updatedAt: "preview",
  },
  {
    id: "local-code-reviewer",
    name: "Local Code Reviewer",
    description: "Deterministic review profile tuned for code, diffs, and concrete findings.",
    systemPrompt:
      "Review code for correctness, regressions, security issues, and missing tests. Lead with actionable findings.",
    sampling: {
      temperature: 0.25,
      topP: 0.9,
      topK: 40,
      minP: 0.03,
      typicalP: 1,
      repeatPenalty: 1.08,
      repeatLastN: 256,
      presencePenalty: 0,
      frequencyPenalty: 0,
      mirostatMode: 0,
      mirostatTau: 5,
      mirostatEta: 0.1,
      seed: null,
      maxTokens: 4096,
      stopSequences: [],
    },
    runtime: {
      backend: "llama.cpp",
      contextLength: 65536,
      batchSize: 1024,
      microBatchSize: 256,
      cpuThreads: 4,
      gpuLayers: 0,
      tensorSplit: [],
      mainGpu: null,
      useMmap: true,
      useMlock: false,
      flashAttention: true,
      kvCacheQuantization: "q8_0",
      ropeFrequencyBase: null,
      ropeFrequencyScale: null,
    },
    output: {
      mode: "text",
      jsonSchema: "",
      grammar: "",
      logitBias: [],
      logprobs: true,
      topLogprobs: 5,
    },
    createdAt: "preview",
    updatedAt: "preview",
  },
  {
    id: "long-context-analyst",
    name: "Long Context Analyst",
    description: "Large-context analysis with compressed KV cache and conservative decoding.",
    systemPrompt:
      "Analyze long context carefully. Track assumptions, cite relevant sections, and avoid inventing missing facts.",
    sampling: {
      temperature: 0.35,
      topP: 0.9,
      topK: 30,
      minP: 0.02,
      typicalP: 1,
      repeatPenalty: 1.12,
      repeatLastN: 512,
      presencePenalty: 0,
      frequencyPenalty: 0.1,
      mirostatMode: 0,
      mirostatTau: 5,
      mirostatEta: 0.1,
      seed: null,
      maxTokens: 8192,
      stopSequences: [],
    },
    runtime: {
      backend: "llama.cpp",
      contextLength: 131072,
      batchSize: 1024,
      microBatchSize: 256,
      cpuThreads: 4,
      gpuLayers: 0,
      tensorSplit: [],
      mainGpu: null,
      useMmap: true,
      useMlock: false,
      flashAttention: true,
      kvCacheQuantization: "q4_0",
      ropeFrequencyBase: null,
      ropeFrequencyScale: null,
    },
    output: {
      mode: "text",
      jsonSchema: "",
      grammar: "",
      logitBias: [],
      logprobs: false,
      topLogprobs: 0,
    },
    createdAt: "preview",
    updatedAt: "preview",
  },
];

async function safeInvoke<T>(command: string, fallback: T, args?: Record<string, unknown>): Promise<T> {
  if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) {
    return fallback;
  }

  try {
    return await invoke<T>(command, args);
  } catch (error) {
    console.warn(`Kivarro command failed: ${command}`, error);
    return fallback;
  }
}

async function invokeOrPreview<T>(
  command: string,
  fallback: T,
  args?: Record<string, unknown>,
): Promise<T> {
  if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) {
    return fallback;
  }

  return invoke<T>(command, args);
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

export function listInferenceProfiles(): Promise<InferenceProfile[]> {
  return safeInvoke("list_inference_profiles", fallbackProfiles);
}

export function saveInferenceProfile(profile: InferenceProfile): Promise<InferenceProfile> {
  return safeInvoke("save_inference_profile", profile, { profile });
}

export function deleteInferenceProfile(id: string): Promise<void> {
  return safeInvoke("delete_inference_profile", undefined, { id });
}

export function getModelLoadPlan(
  modelId: string,
  profile: InferenceProfile,
): Promise<ModelLoadPlan | null> {
  return safeInvoke<ModelLoadPlan | null>("get_model_load_plan", null, { modelId, profile });
}

export function getApiStatus(): Promise<ApiStatus> {
  return safeInvoke("get_api_status", fallbackApiStatus);
}

export function getApiSettings(): Promise<ApiSettings> {
  return safeInvoke("get_api_settings", fallbackApiSettings);
}

export function saveApiSettings(settings: ApiSettings): Promise<ApiStatus> {
  return invokeOrPreview(
    "save_api_settings",
    {
      ...fallbackApiStatus,
      port: settings.port,
      baseUrl: `http://${settings.host}:${settings.port}/v1`,
    },
    { settings },
  );
}

export function getEngineStatus(): Promise<EngineStatus> {
  return safeInvoke("get_engine_status", fallbackEngineStatus);
}

export function startInferenceEngine(
  modelId: string,
  profile: InferenceProfile,
): Promise<EngineStatus> {
  return invokeOrPreview("start_inference_engine", fallbackEngineStatus, { modelId, profile });
}

export function stopInferenceEngine(): Promise<EngineStatus> {
  return invokeOrPreview("stop_inference_engine", fallbackEngineStatus);
}

export function cancelChatCompletionStream(requestId: string): Promise<boolean> {
  return invokeOrPreview("cancel_chat_completion_stream", false, { requestId });
}

export function runChatCompletion(
  modelId: string,
  profile: InferenceProfile,
  prompt: string,
  history: ChatTurn[],
): Promise<InferenceRunResult> {
  return invokeOrPreview(
    "run_chat_completion",
    {
      content: "Preview mode cannot reach a local inference engine. Open Kivarro in Tauri to run this prompt.",
      model: "preview",
      backend: "preview",
      elapsedMs: 0,
      tokensPerSecond: 0,
      promptTokens: null,
      completionTokens: null,
      totalTokens: null,
      finishReason: "preview",
    },
    { modelId, profile, prompt, history },
  );
}

export function runChatCompletionStream(
  requestId: string,
  modelId: string,
  profile: InferenceProfile,
  prompt: string,
  history: ChatTurn[],
): Promise<InferenceRunResult> {
  return invokeOrPreview(
    "run_chat_completion_stream",
    {
      content: "Preview mode cannot stream from a local inference engine. Open Kivarro in Tauri to run this prompt.",
      model: "preview",
      backend: "preview",
      elapsedMs: 0,
      tokensPerSecond: 0,
      promptTokens: null,
      completionTokens: null,
      totalTokens: null,
      finishReason: "preview",
    },
    { requestId, modelId, profile, prompt, history },
  );
}

export function listBenchmarkResults(): Promise<BenchmarkResult[]> {
  return safeInvoke("list_benchmark_results", []);
}

export function runBenchmark(
  modelId: string,
  profile: InferenceProfile,
): Promise<BenchmarkResult[]> {
  return invokeOrPreview("run_benchmark", [], { modelId, profile });
}

export function listKnowledgeBases(): Promise<KnowledgeBase[]> {
  return safeInvoke("list_knowledge_bases", fallbackKnowledgeBases);
}

export function createKnowledgeBase(name: string): Promise<KnowledgeBase[]> {
  return invokeOrPreview("create_knowledge_base", fallbackKnowledgeBases, { name });
}

export function listKnowledgeDocuments(knowledgeBaseId: string): Promise<KnowledgeDocument[]> {
  return safeInvoke("list_knowledge_documents", [], { knowledgeBaseId });
}

export function importKnowledgeDocument(
  knowledgeBaseId: string,
  path: string,
): Promise<KnowledgeBaseDetail> {
  return invokeOrPreview(
    "import_knowledge_document",
    { base: fallbackKnowledgeBases[0], documents: [] },
    { knowledgeBaseId, path },
  );
}

export function testKnowledgeRetrieval(
  knowledgeBaseId: string,
  query: string,
): Promise<RetrievalMatch[]> {
  return invokeOrPreview("test_knowledge_retrieval", [], { knowledgeBaseId, query });
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
