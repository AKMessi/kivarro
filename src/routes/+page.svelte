<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import {
    Activity,
    Bot,
    Boxes,
    BrainCircuit,
    ChevronDown,
    Circle,
    Clipboard,
    Cpu,
    Database,
    FileText,
    FolderOpen,
    Gauge,
    HardDrive,
    Layers3,
    MessageSquare,
    Maximize2,
    Minus,
    Moon,
    PanelLeftClose,
    PanelRightClose,
    Play,
    Power,
    Search,
    Send,
    Server,
    Settings,
    SlidersHorizontal,
    Split,
    Sun,
    Terminal,
    Upload,
    X,
    Zap,
  } from "@lucide/svelte";
  import {
    cancelChatCompletionStream,
    fallbackProfiles,
    createKnowledgeBase,
    getApiSettings,
    getApiStatus,
    getEngineStatus,
    getHardwareSnapshot,
    getModelLoadPlan,
    getRuntimeMetrics,
    importModelFile,
    importKnowledgeDocument,
    listBenchmarkResults,
    listInferenceProfiles,
    listKnowledgeBases,
    listKnowledgeDocuments,
    listModels,
    listSystemLogs,
    runBenchmark,
    runChatCompletionStream,
    saveApiSettings,
    saveInferenceProfile,
    startInferenceEngine,
    stopInferenceEngine,
    testKnowledgeRetrieval,
  } from "$lib/api";
  import type {
    ApiSettings,
    ApiStatus,
    BenchmarkResult,
    ChatTurn,
    EngineStatus,
    HardwareSnapshot,
    InferenceProfile,
    InferenceRunResult,
    InferenceStreamEvent,
    KnowledgeBase,
    KnowledgeDocument,
    LogEntry,
    ModelRecord,
    ModelLoadPlan,
    RetrievalMatch,
    RuntimeMetrics,
    ViewId,
  } from "$lib/types";

  type NavItem = {
    id: ViewId;
    label: string;
    icon: typeof Activity;
  };

  type PaletteAction = {
    id: string;
    label: string;
    detail: string;
    keywords: string[];
    icon: typeof Activity;
    disabled?: boolean;
    run: () => void | Promise<void>;
  };

  type ChatMessage = {
    id: string;
    role: "user" | "assistant" | "system";
    label: string;
    content: string;
    tokens?: number;
    speed?: number;
    streaming?: boolean;
  };

  type RagChunkRow = {
    index: number;
    range: string;
    score: number | null;
    snippet: string;
  };

  const navItems: NavItem[] = [
    { id: "command", label: "Command Center", icon: MessageSquare },
    { id: "models", label: "Model Registry", icon: Boxes },
    { id: "hardware", label: "Hardware Fit", icon: Cpu },
    { id: "tuning", label: "Expert Tuning", icon: SlidersHorizontal },
    { id: "knowledge", label: "Knowledge Base", icon: Database },
    { id: "agents", label: "Agents", icon: Bot },
    { id: "api", label: "Local API", icon: Server },
    { id: "benchmarks", label: "Benchmarks", icon: Gauge },
    { id: "logs", label: "System Logs", icon: Terminal },
    { id: "settings", label: "Settings", icon: Settings },
  ];

  const chatHistory = [
    { title: "Today", items: ["Inference scratchpad", "Schema extraction test", "Qwen coding profile"] },
    { title: "Previous 7 Days", items: ["Long context summary", "RAG retrieval audit"] },
  ];

  const agentTools = [
    { name: "Local terminal", enabled: false, danger: true },
    { name: "Web search", enabled: false, danger: false },
    { name: "Filesystem read", enabled: true, danger: false },
    { name: "MCP registry", enabled: false, danger: false },
  ];

  let activeView: ViewId = "command";
  let theme: "dark" | "light" = "dark";
  let leftCollapsed = false;
  let rightCollapsed = false;
  let commandPaletteOpen = false;
  let paletteQuery = "";
  let paletteInput: HTMLInputElement | null = null;
  let splitView = false;
  let promptText = "";
  let selectedProfileId = fallbackProfiles[0].id;
  let selectedModelId = "";
  let modelFilter = "";
  let logFilter = "ALL";
  let logSearch = "";
  let profileSaveStatus = "Synced";
  let engineBusy = false;
  let engineNotice = "";
  let promptBusy = false;
  let benchmarkBusy = false;
  let knowledgeBusy = false;
  let apiConfigBusy = false;
  let generationCancelling = false;
  let currentStreamRequestId = "";
  let apiCopyStatus = "Copy";
  let modelImportPath = "";
  let modelImportBusy = false;
  let selectedKnowledgeBaseId = "";
  let selectedKnowledgeDocumentId = "";
  let knowledgeImportPath = "";
  let newKnowledgeBaseName = "";
  let retrievalQuery = "";

  let hardware: HardwareSnapshot | null = null;
  let metrics: RuntimeMetrics | null = null;
  let models: ModelRecord[] = [];
  let profiles: InferenceProfile[] = fallbackProfiles;
  let loadPlan: ModelLoadPlan | null = null;
  let apiStatus: ApiStatus | null = null;
  let apiSettings: ApiSettings = { host: "127.0.0.1", port: 8080 };
  let engineStatus: EngineStatus | null = null;
  let benchmarks: BenchmarkResult[] = [];
  let logs: LogEntry[] = [];
  let knowledgeBases: KnowledgeBase[] = [];
  let knowledgeDocuments: KnowledgeDocument[] = [];
  let retrievalResults: RetrievalMatch[] = [];

  let sampling = controlsFromProfile(fallbackProfiles[0]);
  let paletteActions: PaletteAction[] = [];
  let filteredPaletteActions: PaletteAction[] = [];

  let chatMessages: ChatMessage[] = [
    {
      id: "system",
      role: "system",
      label: "Session",
      content: "Initialize session. Select a model to begin.",
    },
    {
      id: "assistant-preview",
      role: "assistant",
      label: "Kivarro",
      content:
        "The command center is online. Drop a GGUF into ./models or open Model Registry to connect a local file.",
      tokens: 28,
      speed: 0,
    },
  ];

  $: activeMeta = navItems.find((item) => item.id === activeView) ?? navItems[0];
  $: activeProfile =
    profiles.find((profile) => profile.id === selectedProfileId) ?? profiles[0] ?? fallbackProfiles[0];
  $: selectedModel = models.find((model) => model.id === selectedModelId) ?? models[0] ?? null;
  $: contextPercent = metrics
    ? clamp((metrics.contextUsedTokens / metrics.contextTotalTokens) * 100, 0, 100)
    : 0;
  $: ramPercent = metrics ? clamp((metrics.ramUsedGib / Math.max(metrics.ramTotalGib, 1)) * 100, 0, 100) : 0;
  $: filteredModels = models.filter((model) =>
    [
      model.name,
      model.format,
      model.path,
      model.architecture,
      model.parameterSize,
      model.quantization,
      model.metadataSource,
    ]
      .filter(Boolean)
      .join(" ")
      .toLowerCase()
      .includes(modelFilter.toLowerCase()),
  );
  $: filteredLogs = logs.filter(
    (entry) => (logFilter === "ALL" || entry.level.toUpperCase() === logFilter) && matchesLogSearch(entry),
  );
  $: activeKnowledgeBase =
    knowledgeBases.find((base) => base.id === selectedKnowledgeBaseId) ?? knowledgeBases[0] ?? null;
  $: if (
    knowledgeDocuments.length > 0 &&
    !knowledgeDocuments.some((document) => document.id === selectedKnowledgeDocumentId)
  ) {
    selectedKnowledgeDocumentId = knowledgeDocuments[0].id;
  }
  $: if (knowledgeDocuments.length === 0 && selectedKnowledgeDocumentId) {
    selectedKnowledgeDocumentId = "";
  }
  $: activeKnowledgeDocument =
    knowledgeDocuments.find((document) => document.id === selectedKnowledgeDocumentId) ?? knowledgeDocuments[0] ?? null;
  $: ragChunkRows = buildRagChunkRows(activeKnowledgeDocument, retrievalResults);
  $: configuredBaseUrl = apiStatus?.baseUrl ?? `http://${apiSettings.host}:${apiSettings.port}/v1`;
  $: profilePreviewJson = JSON.stringify(buildProfileFromControls(), null, 2);
  $: engineOnline = engineStatus?.state === "ready";
  $: engineLoading = engineStatus?.state === "loading";
  $: apiEndpointLocked = engineOnline || engineLoading || Boolean(apiStatus?.enabled);
  $: paletteActions = buildPaletteActions();
  $: filteredPaletteActions = paletteActions.filter((action) => paletteMatches(action, paletteQuery)).slice(0, 14);
  $: engineLabel = engineBusy
    ? "Starting"
    : engineOnline
      ? "Ready"
      : engineLoading
        ? "Loading"
        : "Load model";
  $: document.documentElement.dataset.theme = theme;

  onMount(() => {
    void hydrate();
    let streamUnlisten: (() => void) | null = null;
    let disposed = false;
    if (isTauriRuntime()) {
      void listen<InferenceStreamEvent>("kivarro://chat-stream", (event) => {
        handleStreamEvent(event.payload);
      }).then((unlisten) => {
        if (disposed) {
          unlisten();
        } else {
          streamUnlisten = unlisten;
        }
      });
    }

    const refreshTimer = window.setInterval(() => void refreshRuntime(), 4000);
      const keyHandler = (event: KeyboardEvent) => {
        if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
          event.preventDefault();
          toggleCommandPalette();
        }
      };

    window.addEventListener("keydown", keyHandler);

    return () => {
      disposed = true;
      streamUnlisten?.();
      window.clearInterval(refreshTimer);
      window.removeEventListener("keydown", keyHandler);
    };
  });

  async function hydrate() {
    const [
      nextHardware,
      nextMetrics,
      nextModels,
      nextProfiles,
      nextApiStatus,
      nextApiSettings,
      nextEngineStatus,
      nextBenchmarks,
      nextKnowledgeBases,
      nextLogs,
    ] =
      await Promise.all([
        getHardwareSnapshot(),
        getRuntimeMetrics(),
        listModels(),
        listInferenceProfiles(),
        getApiStatus(),
        getApiSettings(),
        getEngineStatus(),
        listBenchmarkResults(),
        listKnowledgeBases(),
        listSystemLogs(),
      ]);

    hardware = nextHardware;
    metrics = nextMetrics;
    models = nextModels;
    profiles = nextProfiles.length > 0 ? nextProfiles : fallbackProfiles;
    selectedProfileId = profiles[0]?.id ?? fallbackProfiles[0].id;
    sampling = controlsFromProfile(profiles[0] ?? fallbackProfiles[0]);
    selectedModelId = nextModels[0]?.id ?? "";
    apiStatus = nextApiStatus;
    apiSettings = nextApiSettings;
    engineStatus = nextEngineStatus;
    engineNotice = nextEngineStatus.message;
    benchmarks = nextBenchmarks;
    knowledgeBases = nextKnowledgeBases;
    selectedKnowledgeBaseId = nextKnowledgeBases[0]?.id ?? "";
    knowledgeDocuments = selectedKnowledgeBaseId
      ? await listKnowledgeDocuments(selectedKnowledgeBaseId)
      : [];
    logs = nextLogs;
    await updateLoadPlan();
  }

  async function refreshRuntime() {
    const [nextMetrics, nextEngineStatus, nextApiStatus] = await Promise.all([
      getRuntimeMetrics(),
      getEngineStatus(),
      getApiStatus(),
    ]);
    metrics = nextMetrics;
    engineStatus = nextEngineStatus;
    apiStatus = nextApiStatus;
    engineNotice = nextEngineStatus.message;
  }

  async function refreshLogs() {
    logs = await listSystemLogs();
  }

  function setActiveView(view: ViewId) {
    activeView = view;
    commandPaletteOpen = false;
    paletteQuery = "";
  }

  function jumpToWorkspaceSection(sectionId: string) {
    document.getElementById(sectionId)?.scrollIntoView({ block: "start" });
  }

  function toggleCommandPalette() {
    commandPaletteOpen = !commandPaletteOpen;
    if (commandPaletteOpen) {
      paletteQuery = "";
      requestAnimationFrame(() => paletteInput?.focus());
    }
  }

  function closeCommandPalette() {
    commandPaletteOpen = false;
    paletteQuery = "";
  }

  function buildPaletteActions(): PaletteAction[] {
    const navigationActions = navItems.map(
      (item): PaletteAction => ({
        id: `view-${item.id}`,
        label: `Open ${item.label}`,
        detail: "Switch workspace",
        keywords: [item.id, item.label, "view", "workspace", "open"],
        icon: item.icon,
        run: () => setActiveView(item.id),
      }),
    );

    return [
      ...navigationActions,
      {
        id: "theme-toggle",
        label: theme === "dark" ? "Switch to light mode" : "Switch to dark mode",
        detail: `Current theme: ${theme}`,
        keywords: ["theme", "appearance", "light", "dark"],
        icon: theme === "dark" ? Sun : Moon,
        run: toggleTheme,
      },
      {
        id: "save-profile",
        label: "Save inference profile",
        detail: `${activeProfile.name} -> .kivarro.json`,
        keywords: ["save", "profile", "tuning", "json"],
        icon: Clipboard,
        disabled: profileSaveStatus === "Saving",
        run: saveCurrentProfile,
      },
      {
        id: "load-model",
        label: engineOnline ? "Reload selected model" : "Load selected model",
        detail: selectedModel?.name ?? "Select a model in Model Registry",
        keywords: ["load", "model", "engine", "runtime"],
        icon: Play,
        disabled: !selectedModelId || engineBusy,
        run: startSelectedModel,
      },
      {
        id: "stop-engine",
        label: "Stop inference engine",
        detail: engineStatus?.message ?? "No running engine",
        keywords: ["stop", "engine", "unload", "runtime"],
        icon: Power,
        disabled: !engineOnline && !engineLoading,
        run: stopEngine,
      },
      {
        id: "copy-api-url",
        label: "Copy API base URL",
        detail: configuredBaseUrl,
        keywords: ["copy", "api", "url", "endpoint"],
        icon: Clipboard,
        run: copyBaseUrl,
      },
      {
        id: "save-api-settings",
        label: "Save API endpoint",
        detail: `${apiSettings.host}:${apiSettings.port}`,
        keywords: ["api", "settings", "host", "port", "save"],
        icon: Server,
        disabled: apiConfigBusy || apiEndpointLocked,
        run: saveCurrentApiSettings,
      },
      {
        id: "run-benchmark",
        label: "Run benchmark",
        detail: selectedModel?.name ?? "Select and load a model first",
        keywords: ["benchmark", "tokens", "speed", "eval"],
        icon: Gauge,
        disabled: benchmarkBusy || !selectedModelId,
        run: runModelBenchmark,
      },
      {
        id: "refresh-logs",
        label: "Refresh system logs",
        detail: `${logs.length} entries loaded`,
        keywords: ["logs", "refresh", "system", "debug"],
        icon: Terminal,
        run: refreshLogs,
      },
    ];
  }

  function paletteMatches(action: PaletteAction, query: string) {
    const normalized = query.trim().toLowerCase();
    if (!normalized) return true;

    return [action.label, action.detail, ...action.keywords].join(" ").toLowerCase().includes(normalized);
  }

  async function runPaletteAction(action: PaletteAction | undefined) {
    if (!action || action.disabled) return;
    closeCommandPalette();
    await action.run();
  }

  function selectProfile(profileId: string) {
    const profile = profiles.find((candidate) => candidate.id === profileId);
    if (!profile) return;

    selectedProfileId = profile.id;
    sampling = controlsFromProfile(profile);
    profileSaveStatus = "Loaded";
    void updateLoadPlan(profile);
  }

  function selectModel(modelId: string) {
    selectedModelId = modelId;
    void updateLoadPlan();
  }

  async function importModelPath() {
    const path = modelImportPath.trim();
    if (!path || modelImportBusy) return;

    modelImportBusy = true;
    try {
      const result = await importModelFile(path);
      models = result.models.length > 0 ? result.models : [result.imported];
      selectedModelId = result.imported.id;
      modelImportPath = "";
      modelFilter = "";
      await updateLoadPlan();
      await refreshLogs();
      addSystemMessage("Model Registry", `Imported ${result.imported.name}`);
    } catch (error) {
      addSystemMessage("Model Registry", errorMessage(error));
    } finally {
      modelImportBusy = false;
    }
  }

  async function selectKnowledgeBase(knowledgeBaseId: string) {
    selectedKnowledgeBaseId = knowledgeBaseId;
    retrievalResults = [];
    knowledgeDocuments = await listKnowledgeDocuments(knowledgeBaseId);
    selectedKnowledgeDocumentId = knowledgeDocuments[0]?.id ?? "";
  }

  function selectKnowledgeDocument(documentId: string) {
    selectedKnowledgeDocumentId = documentId;
  }

  async function createCurrentKnowledgeBase() {
    const name = newKnowledgeBaseName.trim();
    if (!name || knowledgeBusy) return;

    knowledgeBusy = true;
    try {
      knowledgeBases = await createKnowledgeBase(name);
      const created = knowledgeBases.find((base) => base.name === name) ?? knowledgeBases[0];
      selectedKnowledgeBaseId = created?.id ?? "";
      knowledgeDocuments = selectedKnowledgeBaseId
        ? await listKnowledgeDocuments(selectedKnowledgeBaseId)
        : [];
      selectedKnowledgeDocumentId = knowledgeDocuments[0]?.id ?? "";
      retrievalResults = [];
      newKnowledgeBaseName = "";
      await refreshLogs();
    } catch (error) {
      addSystemMessage("Knowledge", errorMessage(error));
    } finally {
      knowledgeBusy = false;
    }
  }

  async function updateLoadPlan(profile = buildProfileFromControls()) {
    if (!selectedModelId) {
      loadPlan = null;
      return;
    }

    loadPlan = await getModelLoadPlan(selectedModelId, profile);
  }

  async function saveCurrentProfile() {
    profileSaveStatus = "Saving";
    const savedProfile = await saveInferenceProfile(buildProfileFromControls());
    profiles = [
      savedProfile,
      ...profiles.filter((profile) => profile.id !== savedProfile.id),
    ].sort((left, right) => left.name.localeCompare(right.name));
    selectedProfileId = savedProfile.id;
    profileSaveStatus = "Saved";
    void updateLoadPlan(savedProfile);
    void refreshLogs();
  }

  function toggleTheme() {
    theme = theme === "dark" ? "light" : "dark";
  }

  async function startSelectedModel() {
    if (!selectedModelId || !selectedModel) {
      engineNotice = "Select a local model before loading.";
      return;
    }

    const profile = buildProfileFromControls();
    engineBusy = true;
    engineNotice = `Starting ${profile.runtime.backend} for ${selectedModel.name}`;
    try {
      engineStatus = await startInferenceEngine(selectedModelId, profile);
      engineNotice = engineStatus.message;
      const [nextMetrics, nextApiStatus] = await Promise.all([getRuntimeMetrics(), getApiStatus()]);
      metrics = nextMetrics;
      apiStatus = nextApiStatus;
      await refreshLogs();
    } catch (error) {
      engineNotice = errorMessage(error);
      addSystemMessage("Engine", engineNotice);
    } finally {
      engineBusy = false;
    }
  }

  async function loadModelFromRegistry(model: ModelRecord) {
    selectedModelId = model.id;
    await updateLoadPlan();

    const profile = buildProfileFromControls();
    engineBusy = true;
    engineNotice = `Starting ${profile.runtime.backend} for ${model.name}`;
    try {
      engineStatus = await startInferenceEngine(model.id, profile);
      engineNotice = engineStatus.message;
      const [nextMetrics, nextApiStatus] = await Promise.all([getRuntimeMetrics(), getApiStatus()]);
      metrics = nextMetrics;
      apiStatus = nextApiStatus;
      await refreshLogs();
    } catch (error) {
      engineNotice = errorMessage(error);
      addSystemMessage("Engine", engineNotice);
    } finally {
      engineBusy = false;
    }
  }

  async function stopEngine() {
    engineBusy = true;
    try {
      engineStatus = await stopInferenceEngine();
      engineNotice = engineStatus.message;
      const [nextMetrics, nextApiStatus] = await Promise.all([getRuntimeMetrics(), getApiStatus()]);
      metrics = nextMetrics;
      apiStatus = nextApiStatus;
      await refreshLogs();
    } catch (error) {
      engineNotice = errorMessage(error);
      addSystemMessage("Engine", engineNotice);
    } finally {
      engineBusy = false;
    }
  }

  async function saveCurrentApiSettings() {
    if (apiConfigBusy) return;

    const nextSettings: ApiSettings = {
      host: apiSettings.host.trim(),
      port: Number(apiSettings.port),
    };
    apiConfigBusy = true;
    try {
      apiStatus = await saveApiSettings(nextSettings);
      apiSettings = await getApiSettings();
      const [nextMetrics, nextEngineStatus] = await Promise.all([
        getRuntimeMetrics(),
        getEngineStatus(),
      ]);
      metrics = nextMetrics;
      engineStatus = nextEngineStatus;
      engineNotice = nextEngineStatus.message;
      apiCopyStatus = "Saved";
      await refreshLogs();
    } catch (error) {
      addSystemMessage("API", errorMessage(error));
      apiCopyStatus = "Save failed";
    } finally {
      apiConfigBusy = false;
    }
  }

  async function copyBaseUrl() {
    try {
      await navigator.clipboard.writeText(configuredBaseUrl);
      apiCopyStatus = "Copied";
    } catch (error) {
      addSystemMessage("API", `Clipboard unavailable: ${errorMessage(error)}`);
      apiCopyStatus = "Copy failed";
    }
  }

  async function importKnowledgePath() {
    if (!selectedKnowledgeBaseId || !knowledgeImportPath.trim() || knowledgeBusy) return;

    knowledgeBusy = true;
    try {
      const detail = await importKnowledgeDocument(selectedKnowledgeBaseId, knowledgeImportPath);
      knowledgeBases = [
        detail.base,
        ...knowledgeBases.filter((base) => base.id !== detail.base.id),
      ].sort((left, right) => left.name.localeCompare(right.name));
      knowledgeDocuments = detail.documents;
      selectedKnowledgeBaseId = detail.base.id;
      selectedKnowledgeDocumentId = detail.documents[0]?.id ?? "";
      retrievalResults = [];
      knowledgeImportPath = "";
      await refreshLogs();
    } catch (error) {
      addSystemMessage("Knowledge", errorMessage(error));
    } finally {
      knowledgeBusy = false;
    }
  }

  async function runRetrievalTest() {
    if (!selectedKnowledgeBaseId || !retrievalQuery.trim() || knowledgeBusy) return;

    knowledgeBusy = true;
    try {
      retrievalResults = await testKnowledgeRetrieval(selectedKnowledgeBaseId, retrievalQuery);
    } catch (error) {
      retrievalResults = [];
      addSystemMessage("Knowledge", errorMessage(error));
    } finally {
      knowledgeBusy = false;
    }
  }

  async function runModelBenchmark() {
    if (!selectedModelId || !selectedModel) {
      addSystemMessage("Benchmark", "Select and load a model before running a benchmark.");
      activeView = "models";
      return;
    }
    if (!engineOnline) {
      addSystemMessage("Benchmark", "Load the selected model before running a benchmark.");
      return;
    }

    benchmarkBusy = true;
    try {
      benchmarks = await runBenchmark(selectedModelId, buildProfileFromControls());
      const [nextMetrics, nextEngineStatus] = await Promise.all([
        getRuntimeMetrics(),
        getEngineStatus(),
      ]);
      metrics = nextMetrics;
      engineStatus = nextEngineStatus;
      await refreshLogs();
    } catch (error) {
      addSystemMessage("Benchmark", errorMessage(error));
    } finally {
      benchmarkBusy = false;
    }
  }

  async function submitPrompt() {
    const prompt = promptText.trim();
    if (!prompt || promptBusy) return;
    if (!selectedModelId) {
      addSystemMessage("Engine", "Select and load a model before sending a prompt.");
      return;
    }
    const requestId = createId("assistant");

    const history = chatMessages
      .filter(
        (message) =>
          (message.role === "user" || message.role === "assistant") &&
          message.id !== "assistant-preview" &&
          message.content.trim().length > 0,
      )
      .slice(-12)
      .map((message): ChatTurn => ({ role: message.role, content: message.content }));

    chatMessages = [
      ...chatMessages,
      {
        id: createId("user"),
        role: "user",
        label: "You",
        content: prompt,
      },
      {
        id: requestId,
        role: "assistant",
        label: selectedModel?.name ?? "Kivarro",
        content: "",
        tokens: 0,
        speed: 0,
        streaming: true,
      },
    ];
    promptText = "";
    promptBusy = true;
    generationCancelling = false;
    currentStreamRequestId = requestId;

    try {
      const result = await runChatCompletionStream(
        requestId,
        selectedModelId,
        buildProfileFromControls(),
        prompt,
        history,
      );
      finalizeAssistantResult(requestId, result);
      const [nextMetrics, nextEngineStatus, nextApiStatus] = await Promise.all([
        getRuntimeMetrics(),
        getEngineStatus(),
        getApiStatus(),
      ]);
      metrics = nextMetrics;
      engineStatus = nextEngineStatus;
      apiStatus = nextApiStatus;
    } catch (error) {
      const message = errorMessage(error);
      if (message.toLowerCase().includes("stream cancelled")) {
        markAssistantCancelled(requestId);
      } else {
        markAssistantError(requestId, message);
      }
    } finally {
      promptBusy = false;
      generationCancelling = false;
      currentStreamRequestId = "";
    }
  }

  async function minimizeWindow() {
    await getTauriWindow()?.minimize().catch(() => undefined);
  }

  async function toggleMaximizeWindow() {
    await getTauriWindow()?.toggleMaximize().catch(() => undefined);
  }

  async function closeWindow() {
    await getTauriWindow()?.close().catch(() => undefined);
  }

  function getTauriWindow() {
    if (!isTauriRuntime()) {
      return null;
    }

    return getCurrentWindow();
  }

  function isTauriRuntime() {
    return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  }

  function clamp(value: number, min: number, max: number) {
    return Math.min(Math.max(value, min), max);
  }

  function formatNumber(value: number, digits = 1) {
    return Number.isFinite(value) ? value.toFixed(digits) : "0.0";
  }

  function formatTokens(value: number) {
    return new Intl.NumberFormat("en-US").format(value);
  }

  function formatBytes(value: number) {
    if (!Number.isFinite(value) || value <= 0) return "0 B";

    const units = ["B", "KB", "MB", "GB", "TB"];
    let size = value;
    let unitIndex = 0;
    while (size >= 1024 && unitIndex < units.length - 1) {
      size /= 1024;
      unitIndex += 1;
    }

    return `${size >= 10 || unitIndex === 0 ? size.toFixed(0) : size.toFixed(1)} ${units[unitIndex]}`;
  }

  function formatShortDate(value: string) {
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return value || "unknown";
    return new Intl.DateTimeFormat("en-US", { month: "short", day: "2-digit" }).format(date);
  }

  function optionalText(value: string | null | undefined, fallback = "unknown") {
    const trimmed = value?.trim();
    return trimmed ? trimmed : fallback;
  }

  function formatTokenLimit(value: number | null | undefined) {
    return value && value > 0 ? formatTokens(value) : "unknown";
  }

  function formatLayerCount(value: number | null | undefined) {
    return value && value > 0 ? `${value}` : "estimated";
  }

  function scopePoints(utilization: number | null | undefined, channel = 0) {
    const base = 100 - clamp(utilization ?? 0, 0, 100);
    return Array.from({ length: 16 }, (_, index) => {
      const x = (index / 15) * 100;
      const phase = index * 0.72 + channel * 0.9;
      const pulse = Math.sin(phase) * 9 + Math.cos(phase * 0.53) * 5;
      const y = clamp(base + pulse, 8, 92);
      return `${formatNumber(x, 2)},${formatNumber(y, 2)}`;
    }).join(" ");
  }

  function benchmarkBarWidth(value: number | null | undefined) {
    const peak = Math.max(...benchmarks.map((result) => result.tokensPerSecond), 1);
    return clamp(((value ?? 0) / peak) * 100, 2, 100);
  }

  function buildRagChunkRows(
    document: KnowledgeDocument | null,
    matches: RetrievalMatch[],
  ): RagChunkRow[] {
    if (!document || document.chunkCount <= 0) return [];

    const matchesByChunk = new Map(
      matches
        .filter((match) => match.documentId === document.id)
        .map((match) => [match.chunkIndex, match]),
    );
    const visibleCount = Math.min(document.chunkCount, 24);

    return Array.from({ length: visibleCount }, (_, index) => {
      const match = matchesByChunk.get(index);
      const start = index * 1200;
      const end = start + 1199;
      return {
        index,
        range: `${formatTokens(start)}-${formatTokens(end)}`,
        score: match?.score ?? null,
        snippet:
          match?.snippet ??
          `Indexed chunk ${index + 1} from ${document.name}. Run a retrieval query to inspect ranked content from this segment.`,
      };
    });
  }

  function createId(prefix: string) {
    return `${prefix}-${globalThis.crypto?.randomUUID?.() ?? Date.now().toString(36)}`;
  }

  function errorMessage(error: unknown) {
    if (typeof error === "string") return error;
    if (error instanceof Error) return error.message;
    return "Local engine command failed.";
  }

  function matchesLogSearch(entry: LogEntry) {
    const query = logSearch.trim();
    if (!query) return true;
    const haystack = `${entry.timestamp} ${entry.level} ${entry.source} ${entry.message}`;
    try {
      return new RegExp(query, "i").test(haystack);
    } catch {
      return haystack.toLowerCase().includes(query.toLowerCase());
    }
  }

  function addSystemMessage(label: string, content: string) {
    chatMessages = [
      ...chatMessages,
      {
        id: createId("system"),
        role: "system",
        label,
        content,
      },
    ];
  }

  function handleStreamEvent(event: InferenceStreamEvent) {
    chatMessages = chatMessages.map((message) => {
      if (message.id !== event.requestId) return message;

      return {
        ...message,
        label: event.model || message.label,
        content: event.content || message.content,
        tokens: event.completionTokens || message.tokens,
        speed: event.tokensPerSecond || message.speed,
        streaming: event.phase !== "done" && event.phase !== "error" && event.phase !== "cancelled",
      };
    });
  }

  function finalizeAssistantResult(messageId: string, result: InferenceRunResult) {
    chatMessages = chatMessages.map((message) => {
      if (message.id !== messageId) return message;

      return {
        ...message,
        label: result.model,
        content: result.content || message.content,
        tokens: result.completionTokens ?? message.tokens,
        speed: result.tokensPerSecond,
        streaming: false,
      };
    });
  }

  function markAssistantError(messageId: string, content: string) {
    chatMessages = chatMessages.map((message) => {
      if (message.id !== messageId) return message;

      return {
        ...message,
        label: "Engine",
        content,
        streaming: false,
      };
    });
  }

  function markAssistantCancelled(messageId: string) {
    chatMessages = chatMessages.map((message) => {
      if (message.id !== messageId) return message;

      return {
        ...message,
        content: message.content || "Generation stopped before the first token.",
        streaming: false,
      };
    });
  }

  async function cancelCurrentStream() {
    if (!currentStreamRequestId || generationCancelling) return;

    generationCancelling = true;
    try {
      const cancelled = await cancelChatCompletionStream(currentStreamRequestId);
      if (!cancelled) {
        addSystemMessage("Engine", "No active stream matched the current request.");
      }
    } catch (error) {
      addSystemMessage("Engine", errorMessage(error));
    }
  }

  function controlsFromProfile(profile: InferenceProfile) {
    return {
      temperature: profile.sampling.temperature,
      topP: profile.sampling.topP,
      topK: profile.sampling.topK,
      minP: profile.sampling.minP,
      typicalP: profile.sampling.typicalP,
      repeatPenalty: profile.sampling.repeatPenalty,
      repeatLastN: profile.sampling.repeatLastN,
      presencePenalty: profile.sampling.presencePenalty,
      frequencyPenalty: profile.sampling.frequencyPenalty,
      mirostatMode: profile.sampling.mirostatMode,
      mirostatTau: profile.sampling.mirostatTau,
      mirostatEta: profile.sampling.mirostatEta,
      seed: profile.sampling.seed,
      maxTokens: profile.sampling.maxTokens,
      stopSequences: [...profile.sampling.stopSequences],
      backend: profile.runtime.backend,
      contextLength: profile.runtime.contextLength,
      batchSize: profile.runtime.batchSize,
      microBatchSize: profile.runtime.microBatchSize,
      cpuThreads: profile.runtime.cpuThreads,
      gpuLayers: profile.runtime.gpuLayers,
      tensorSplit: [...profile.runtime.tensorSplit],
      mainGpu: profile.runtime.mainGpu,
      useMmap: profile.runtime.useMmap,
      useMlock: profile.runtime.useMlock,
      flashAttention: profile.runtime.flashAttention,
      kvCacheQuantization: profile.runtime.kvCacheQuantization,
      ropeFrequencyBase: profile.runtime.ropeFrequencyBase,
      ropeFrequencyScale: profile.runtime.ropeFrequencyScale,
    };
  }

  function buildProfileFromControls(): InferenceProfile {
    return {
      ...activeProfile,
      sampling: {
        ...activeProfile.sampling,
        temperature: Number(sampling.temperature),
        topP: Number(sampling.topP),
        topK: Number(sampling.topK),
        minP: Number(sampling.minP),
        typicalP: Number(sampling.typicalP),
        repeatPenalty: Number(sampling.repeatPenalty),
        repeatLastN: Number(sampling.repeatLastN),
        presencePenalty: Number(sampling.presencePenalty),
        frequencyPenalty: Number(sampling.frequencyPenalty),
        mirostatMode: Number(sampling.mirostatMode),
        mirostatTau: Number(sampling.mirostatTau),
        mirostatEta: Number(sampling.mirostatEta),
        seed: sampling.seed === null ? null : Number(sampling.seed),
        maxTokens: Number(sampling.maxTokens),
        stopSequences: [...sampling.stopSequences],
      },
      runtime: {
        ...activeProfile.runtime,
        backend: sampling.backend,
        contextLength: Number(sampling.contextLength),
        batchSize: Number(sampling.batchSize),
        microBatchSize: Number(sampling.microBatchSize),
        cpuThreads: Number(sampling.cpuThreads),
        gpuLayers: Number(sampling.gpuLayers),
        tensorSplit: [...sampling.tensorSplit],
        mainGpu: sampling.mainGpu === null ? null : Number(sampling.mainGpu),
        useMmap: Boolean(sampling.useMmap),
        useMlock: Boolean(sampling.useMlock),
        flashAttention: Boolean(sampling.flashAttention),
        kvCacheQuantization: sampling.kvCacheQuantization,
        ropeFrequencyBase:
          sampling.ropeFrequencyBase === null ? null : Number(sampling.ropeFrequencyBase),
        ropeFrequencyScale:
          sampling.ropeFrequencyScale === null ? null : Number(sampling.ropeFrequencyScale),
      },
    };
  }
</script>

<svelte:head>
  <title>Kivarro</title>
  <meta
    name="description"
    content="Kivarro is a local model inference workstation for tuning, serving, and inspecting private AI models."
  />
</svelte:head>

<div class="app">
  <header class="titlebar" data-tauri-drag-region>
    <div class="title-identity" data-tauri-drag-region>
      <span class="brand-lockup">
        <BrainCircuit size={16} strokeWidth={1.8} />
        <span class="wordmark">Kivarro</span>
      </span>
      <span class="title-model">
        <span>{metrics?.activeModel ?? selectedModel?.name ?? "No model loaded"}</span>
        <code>{formatTokens(metrics?.contextUsedTokens ?? 0)} / {formatTokens(metrics?.contextTotalTokens ?? sampling.contextLength)} ctx</code>
        <i class="title-context-meter"><b style={`width: ${contextPercent}%`}></b></i>
      </span>
    </div>

    <button class="title-command" data-tauri-drag-region="false" onclick={toggleCommandPalette}>
      <Search size={13} />
      <span>Command / action</span>
      <code>Ctrl K</code>
    </button>

    <div class="title-actions" data-tauri-drag-region>
      <div class="quick-actions" data-tauri-drag-region="false">
        <button class="icon-button" aria-label="Toggle left panel" title="Toggle left panel" onclick={() => (leftCollapsed = !leftCollapsed)}>
          <PanelLeftClose size={16} />
        </button>
        <button class="icon-button" aria-label="Command palette" title="Cmd/Ctrl + K" onclick={toggleCommandPalette}>
          <Search size={16} />
        </button>
        <button class="icon-button" aria-label="Toggle theme" title="Toggle theme" onclick={toggleTheme}>
          {#if theme === "dark"}
            <Moon size={16} />
          {:else}
            <Sun size={16} />
          {/if}
        </button>
        <button class="icon-button" aria-label="Toggle inspector" title="Toggle inspector" onclick={() => (rightCollapsed = !rightCollapsed)}>
          <PanelRightClose size={16} />
        </button>
      </div>
      <div class="window-controls" data-tauri-drag-region="false">
        <button aria-label="Minimize window" class="window-control minimize" onclick={minimizeWindow}>
          <Minus size={12} />
        </button>
        <button aria-label="Maximize window" class="window-control maximize" onclick={toggleMaximizeWindow}>
          <Maximize2 size={11} />
        </button>
        <button aria-label="Close window" class="window-control close" onclick={closeWindow}>
          <X size={12} />
        </button>
      </div>
    </div>
  </header>

  <div
    class="shell"
    class:left-collapsed={leftCollapsed}
    class:right-collapsed={rightCollapsed}
    class:wide-workspace={activeView === "logs"}
  >
    <nav class="nav-rail" aria-label="Primary navigation">
      <div class="rail-stack">
        {#each navItems as item}
          <button
            class:active={activeView === item.id}
            class="rail-button"
            aria-label={item.label}
            title={item.label}
            onclick={() => setActiveView(item.id)}
          >
            <svelte:component this={item.icon} size={24} strokeWidth={1.7} />
          </button>
        {/each}
      </div>
      <button class="rail-button monitor" aria-label="Hardware status monitor" title="Hardware status">
        <Activity size={24} strokeWidth={1.7} />
      </button>
    </nav>

    <aside class="context-panel">
      <div class="panel-header">
        <span>{activeMeta.label}</span>
        <ChevronDown size={14} />
      </div>

      {#if activeView === "command"}
        <div class="profile-select">
          <label for="profile">Prompt profile</label>
          <select
            id="profile"
            bind:value={selectedProfileId}
            onchange={(event) => selectProfile(event.currentTarget.value)}
          >
            {#each profiles as profile}
              <option value={profile.id}>{profile.name}</option>
            {/each}
          </select>
          <p class="profile-description">{activeProfile.description}</p>
          <code>{activeProfile.id}.kivarro.json</code>
        </div>
        <div class="section-label">Chats</div>
        {#each chatHistory as group}
          <div class="history-group">
            <div class="history-title">{group.title}</div>
            {#each group.items as item}
              <button class="history-item">{item}</button>
            {/each}
          </div>
        {/each}
      {:else if activeView === "models"}
        <div class="search-box">
          <Search size={14} />
          <input placeholder="Filter local models" bind:value={modelFilter} />
        </div>
        <div class="model-import-box">
          <label for="model-import-path">Import model file</label>
          <div>
            <input
              id="model-import-path"
              placeholder="Paste path to .gguf, .mlx, .bin, or .safetensors"
              bind:value={modelImportPath}
              onkeydown={(event) => {
                if (event.key === "Enter") void importModelPath();
              }}
            />
            <button
              aria-label="Import model file"
              disabled={modelImportBusy || !modelImportPath.trim()}
              onclick={importModelPath}
            >
              <Upload size={15} />
            </button>
          </div>
          <small>{modelImportBusy ? "Copying into ./models" : "Files are copied into the local model library."}</small>
        </div>
        <div class="section-label">Discovered</div>
        {#if models.length === 0}
          <p class="muted-copy">No models found in ./models yet.</p>
        {:else}
          {#each filteredModels as model}
            <button class:active={selectedModelId === model.id} class="model-mini" onclick={() => selectModel(model.id)}>
              <span>{model.name}</span>
              <small>
                {optionalText(model.architecture)} / {optionalText(model.quantization, model.format)} /
                {formatTokenLimit(model.contextLength)} ctx
              </small>
            </button>
          {/each}
        {/if}
      {:else if activeView === "hardware"}
        <div class="metric-stack">
          <div>
            <span>CPU</span>
            <strong>{hardware?.cpuCores ?? 0} cores</strong>
          </div>
          <div>
            <span>RAM</span>
            <strong>{formatNumber(hardware?.ramUsedGib ?? 0)} / {formatNumber(hardware?.ramTotalGib ?? 0)} GiB</strong>
          </div>
          <div>
            <span>Platform</span>
            <strong>{hardware?.os ?? "unknown"} / {hardware?.architecture ?? "unknown"}</strong>
          </div>
        </div>
      {:else if activeView === "knowledge"}
        <div class="section-label">Knowledge bases</div>
        <div class="kb-create-row">
          <input
            aria-label="New knowledge base"
            placeholder="New base name"
            bind:value={newKnowledgeBaseName}
            onkeydown={(event) => {
              if (event.key === "Enter") void createCurrentKnowledgeBase();
            }}
          />
          <button class="tool-button" disabled={knowledgeBusy || !newKnowledgeBaseName.trim()} onclick={createCurrentKnowledgeBase}>
            <Database size={14} />
          </button>
        </div>
        {#each knowledgeBases as base}
          <button
            class:active={selectedKnowledgeBaseId === base.id}
            class="history-item"
            onclick={() => void selectKnowledgeBase(base.id)}
          >
            <Database size={14} />
            <span>{base.name}</span>
            <code>{base.chunkCount}</code>
          </button>
        {/each}
        <div class="section-label">Document tree</div>
        {#if knowledgeDocuments.length === 0}
          <p class="muted-copy">No documents indexed in this base.</p>
        {:else}
          <div class="document-tree-list">
            {#each knowledgeDocuments as document}
              <button
                class:active={selectedKnowledgeDocumentId === document.id}
                onclick={() => selectKnowledgeDocument(document.id)}
              >
                <FileText size={14} />
                <span>{document.name}</span>
                <code>{document.chunkCount}</code>
              </button>
            {/each}
          </div>
        {/if}
      {:else if activeView === "tuning"}
        <div class="section-label">Saved profiles</div>
        <div class="profile-context-list">
          {#each profiles as profile}
            <button class:active={selectedProfileId === profile.id} class="profile-context-card" onclick={() => selectProfile(profile.id)}>
              <span>{profile.name}</span>
              <small>{profile.description}</small>
              <code>{profile.id}</code>
            </button>
          {/each}
        </div>
        <div class="section-label">Control groups</div>
        <div class="jump-list">
          <button onclick={() => jumpToWorkspaceSection("tuning-sampling")}>Sampling</button>
          <button onclick={() => jumpToWorkspaceSection("tuning-penalties")}>Penalties</button>
          <button onclick={() => jumpToWorkspaceSection("tuning-runtime")}>Runtime</button>
          <button onclick={() => jumpToWorkspaceSection("tuning-json")}>Live JSON</button>
        </div>
      {:else if activeView === "agents"}
        <div class="section-label">Agent workspace</div>
        <div class="context-stat">
          <span>Draft agent</span>
          <code>Analyst Agent</code>
        </div>
        <div class="context-stat">
          <span>Tool gates</span>
          <code>{agentTools.length} configured</code>
        </div>
        <div class="jump-list">
          <button onclick={() => jumpToWorkspaceSection("agent-contract")}>System Contract</button>
          <button onclick={() => jumpToWorkspaceSection("agent-tools")}>Tool Schemas</button>
        </div>
      {:else if activeView === "api"}
        <div class="section-label">Gateway</div>
        <div class="context-stat">
          <span>Status</span>
          <code>{apiStatus?.enabled ? "running" : "stopped"}</code>
        </div>
        <div class="context-stat">
          <span>Base URL</span>
          <code>{configuredBaseUrl}</code>
        </div>
        <div class="context-stat">
          <span>Routes</span>
          <code>{apiStatus?.endpoints?.length ?? 0}</code>
        </div>
      {:else if activeView === "benchmarks"}
        <div class="section-label">Run summary</div>
        <div class="context-stat">
          <span>Runs</span>
          <code>{formatTokens(benchmarks.length)}</code>
        </div>
        <div class="context-stat">
          <span>Latest tok/s</span>
          <code>{formatNumber(benchmarks[0]?.tokensPerSecond ?? 0)}</code>
        </div>
        <div class="context-stat">
          <span>Active model</span>
          <code>{metrics?.activeModel ?? "none"}</code>
        </div>
      {:else if activeView === "settings"}
        <div class="section-label">Settings categories</div>
        <div class="jump-list">
          <button onclick={() => jumpToWorkspaceSection("settings-general")}>General</button>
          <button onclick={() => jumpToWorkspaceSection("settings-appearance")}>Appearance</button>
          <button onclick={() => jumpToWorkspaceSection("settings-storage")}>Storage Paths</button>
          <button onclick={() => jumpToWorkspaceSection("settings-advanced")}>Advanced</button>
        </div>
        <div class="context-stat">
          <span>Theme</span>
          <code>{theme}</code>
        </div>
        <div class="context-stat">
          <span>API base</span>
          <code>{configuredBaseUrl}</code>
        </div>
      {:else if activeView === "logs"}
        <div class="filter-row">
          {#each ["ALL", "INFO", "WARN", "ERROR", "DEBUG"] as level}
            <button class:active={logFilter === level} onclick={() => (logFilter = level)}>{level}</button>
          {/each}
        </div>
      {:else}
        <div class="section-label">Workspace</div>
        <p class="muted-copy">
          This panel is reserved for fast navigation, saved assets, and local configuration for the active view.
        </p>
      {/if}
    </aside>

    <main class="workspace">
      {#if activeView === "command"}
        <section class="workspace-header">
          <div>
            <p class="eyebrow">Command Center</p>
            <h1>Local inference workbench</h1>
          </div>
          <div class="header-actions">
            <button class:active={splitView} class="tool-button" onclick={() => (splitView = !splitView)}>
              <Split size={15} />
              Split view
            </button>
            <button class:online={engineOnline} class="primary-button" disabled={engineBusy} onclick={startSelectedModel}>
              <Play size={15} />
              {engineLabel}
            </button>
          </div>
        </section>

        <section class:split={splitView} class="chat-surface">
          <div class="chat-pane">
            <div class="pane-header">
              <span>{metrics?.activeModel ?? "No model loaded"}</span>
              <code>{engineStatus?.state ?? "offline"} / {formatNumber(metrics?.tokensPerSecond ?? 0)} tok/s</code>
            </div>
            <div class="message-list">
              {#each chatMessages as message}
                <article
                  class:user={message.role === "user"}
                  class:assistant={message.role === "assistant"}
                  class:system={message.role === "system"}
                  class:streaming={message.streaming}
                  class="message"
                >
                  <div class="message-meta">
                    <span>{message.label}</span>
                    {#if message.tokens}
                      <code>{message.tokens} tokens</code>
                    {/if}
                  </div>
                  <p>
                    {message.content || (message.streaming ? "receiving tokens..." : "")}
                    {#if message.streaming}
                      <span class="stream-cursor"></span>
                    {/if}
                  </p>
                </article>
              {/each}
            </div>
          </div>

          {#if splitView}
            <div class="chat-pane compare">
              <div class="pane-header">
                <span>Comparison lane</span>
                <code>idle</code>
              </div>
              <div class="empty-state compact">
                <Boxes size={28} />
                <strong>No comparison model loaded</strong>
                <span>Load a second model to stream responses side by side.</span>
              </div>
            </div>
          {/if}
        </section>

        <section class="prompt-dock">
          <div class="context-meter">
            <span style={`width: ${contextPercent}%`}></span>
            <i class="marker marker-4k"></i>
            <i class="marker marker-8k"></i>
            <i class="marker marker-16k"></i>
            <i class="marker marker-32k"></i>
          </div>
          <div class="prompt-row">
            <textarea
              aria-label="Prompt input"
              placeholder="Message Kivarro or paste a benchmark prompt..."
              bind:value={promptText}
              onkeydown={(event) => {
                if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
                  event.preventDefault();
                  if (promptBusy) {
                    void cancelCurrentStream();
                  } else {
                    void submitPrompt();
                  }
                }
              }}
            ></textarea>
            <div class="prompt-tools">
              <select aria-label="Prompt profile" bind:value={selectedProfileId} onchange={(event) => selectProfile(event.currentTarget.value)}>
                {#each profiles as profile}
                  <option value={profile.id}>{profile.name}</option>
                {/each}
              </select>
              <label>
                <span>Temp</span>
                <input type="range" min="0" max="1" step="0.01" bind:value={sampling.temperature} />
                <code>{formatNumber(sampling.temperature, 2)}</code>
              </label>
            </div>
            <button
              class:stopping={promptBusy}
              class="send-button"
              aria-label={promptBusy ? "Stop generation" : "Send prompt"}
              aria-busy={generationCancelling}
              disabled={generationCancelling}
              title={promptBusy ? "Stop generation" : "Send prompt"}
              onclick={() => {
                if (promptBusy) {
                  void cancelCurrentStream();
                } else {
                  void submitPrompt();
                }
              }}
            >
              {#if promptBusy}
                <X size={18} />
              {:else}
                <Send size={18} />
              {/if}
            </button>
          </div>
        </section>
      {:else if activeView === "models"}
        <section class="workspace-header">
          <div>
            <p class="eyebrow">Model Registry</p>
            <h1>Local model inventory</h1>
          </div>
          <div class="segmented">
            <button class="active">List</button>
            <button>Grid</button>
          </div>
        </section>

        {#if filteredModels.length === 0}
          <section class="empty-state">
            <HardDrive size={44} />
            <strong>No models found</strong>
            <span>Drop a .gguf file into ./models or import one from the registry workflow.</span>
          </section>
        {:else}
          <section class="model-table">
            <div class="table-row header">
              <span>Status</span>
              <span>Model</span>
              <span>Arch</span>
              <span>Quant</span>
              <span>Size</span>
              <span>RAM Req</span>
              <span>Actions</span>
            </div>
            {#each filteredModels as model}
              <div
                class:active={selectedModelId === model.id}
                class="table-row model-row"
                role="button"
                tabindex="0"
                onclick={() => selectModel(model.id)}
                onkeydown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    selectModel(model.id);
                  }
                }}
              >
                <span class:online-dot={engineStatus?.activeModelId === model.id} class="model-status-dot"></span>
                <span class="model-name-cell">
                  <strong>{model.name}</strong>
                  <small>{model.metadataSource}</small>
                </span>
                <code>{optionalText(model.architecture)}</code>
                <code class="quant-badge">{optionalText(model.quantization, model.format)}</code>
                <code>{formatNumber(model.sizeGib)} GiB</code>
                <span class:good={model.fit === "Fits"} class:warn={model.fit !== "Fits"}>{model.fit}</span>
                <button
                  class:danger={engineStatus?.activeModelId === model.id}
                  class="row-action"
                  disabled={engineBusy}
                  onclick={(event) => {
                    event.stopPropagation();
                    void loadModelFromRegistry(model);
                  }}
                >
                  {engineStatus?.activeModelId === model.id ? "Restart" : "Load"}
                </button>
              </div>
            {/each}
          </section>
        {/if}

        {#if loadPlan}
          <section class="load-plan">
            <div class="panel-header inline">
              <span>Load plan / {selectedModel?.name ?? "selected model"}</span>
              <code>{loadPlan.backend}</code>
            </div>
            <div class="load-plan-grid">
              <div>
                <span>Fit</span>
                <strong>{loadPlan.fit}</strong>
              </div>
              <div>
                <span>Metadata</span>
                <strong>{loadPlan.metadataSource}</strong>
              </div>
              <div>
                <span>Architecture</span>
                <strong>{optionalText(loadPlan.architecture)}</strong>
              </div>
              <div>
                <span>Quantization</span>
                <strong>{optionalText(loadPlan.quantization)}</strong>
              </div>
              <div>
                <span>Model context</span>
                <strong>{formatTokenLimit(loadPlan.modelContextLength)}</strong>
              </div>
              <div>
                <span>Layers</span>
                <strong>{loadPlan.gpuLayers} GPU / {loadPlan.cpuLayers} CPU</strong>
              </div>
              <div>
                <span>Total required</span>
                <strong>{formatNumber(loadPlan.totalRequiredGib)} GiB</strong>
              </div>
              <div>
                <span>Available RAM</span>
                <strong>{formatNumber(loadPlan.ramAvailableGib)} GiB</strong>
              </div>
            </div>
            <div class="segment-bar">
              {#each loadPlan.segments.slice(0, 3) as segment}
                <span
                  class={`segment-${segment.color}`}
                  style={`width: ${(segment.gib / Math.max(loadPlan.totalRequiredGib, 1)) * 100}%`}
                  title={`${segment.label}: ${formatNumber(segment.gib)} GiB`}
                ></span>
              {/each}
            </div>
            <p>{loadPlan.recommendation}</p>
          </section>
        {/if}
      {:else if activeView === "hardware"}
        <section class="workspace-header">
          <div>
            <p class="eyebrow">Silicon Blueprint</p>
            <h1>Hardware fit simulator</h1>
          </div>
          <button class="tool-button">
            <Zap size={15} />
            Auto tune
          </button>
        </section>

        <section class="telemetry-board">
          {#each hardware?.blocks ?? [] as block, index}
            <article class="scope-panel">
              <div class="scope-head">
                <span>{block.kind}</span>
                <strong>{formatNumber(block.utilizationPercent)}%</strong>
              </div>
              <svg class="scope-line" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
                <polyline points={scopePoints(block.utilizationPercent, index)}></polyline>
              </svg>
              <div class="scope-axis">
                <span>0s</span>
                <span>5s</span>
                <span>10s</span>
              </div>
              <div class="scope-meta">
                <strong>{block.name}</strong>
                <span>{block.status}</span>
              </div>
              {#if block.memoryTotalGib}
                <div class="memory-bar">
                  <span style={`width: ${((block.memoryUsedGib ?? 0) / block.memoryTotalGib) * 100}%`}></span>
                </div>
                <code>{formatNumber(block.memoryUsedGib ?? 0)} / {formatNumber(block.memoryTotalGib)} GiB</code>
              {/if}
            </article>
          {/each}
        </section>

        <section class="control-band">
          <div>
            <label for="gpu-layers">GPU offload layers</label>
            <div class="precise-slider">
              <input
                id="gpu-layers"
                type="range"
                min="0"
                max={loadPlan?.estimatedLayers ?? 96}
                step="1"
                bind:value={sampling.gpuLayers}
                onchange={() => void updateLoadPlan()}
              />
              <input
                aria-label="GPU offload layers value"
                class="number-input"
                type="number"
                min="0"
                max={loadPlan?.estimatedLayers ?? 96}
                step="1"
                bind:value={sampling.gpuLayers}
                onchange={() => void updateLoadPlan()}
              />
            </div>
            <code>{loadPlan?.estimatedLayers ?? 96} layers / {loadPlan?.metadataSource?.startsWith("GGUF") ? "from GGUF" : "estimated"}</code>
          </div>
          <div>
            <label for="context-length">Context length</label>
            <div class="precise-slider">
              <input
                id="context-length"
                type="range"
                min="4096"
                max="262144"
                step="4096"
                bind:value={sampling.contextLength}
                onchange={() => void updateLoadPlan()}
              />
              <input
                aria-label="Context length value"
                class="number-input"
                type="number"
                min="4096"
                max="262144"
                step="4096"
                bind:value={sampling.contextLength}
                onchange={() => void updateLoadPlan()}
              />
            </div>
            <code>{formatTokens(sampling.contextLength)} tokens</code>
          </div>
        </section>

        {#if loadPlan}
          <section class="hardware-plan">
            <div class="panel-header inline">
              <span>Simulated allocation</span>
              <code>{loadPlan.fit}</code>
            </div>
            <div class="allocation-grid">
              {#each loadPlan.segments as segment}
                <div>
                  <span>{segment.label}</span>
                  <strong>{formatNumber(segment.gib)} GiB</strong>
                </div>
              {/each}
            </div>
            <div class="segment-bar large">
              {#each loadPlan.segments.slice(0, 3) as segment}
                <span
                  class={`segment-${segment.color}`}
                  style={`width: ${(segment.gib / Math.max(loadPlan.totalRequiredGib, 1)) * 100}%`}
                ></span>
              {/each}
            </div>
            <p>{loadPlan.recommendation}</p>
          </section>
        {/if}
      {:else if activeView === "tuning"}
        <section class="workspace-header">
          <div>
            <p class="eyebrow">Expert Tuning</p>
            <h1>Reusable inference profile</h1>
          </div>
          <button class="primary-button" onclick={saveCurrentProfile}>
            <Clipboard size={15} />
            Save .kivarro.json
          </button>
        </section>

        <section class="profile-strip">
          <div>
            <span>Active profile</span>
            <strong>{activeProfile.name}</strong>
            <code>{profileSaveStatus}</code>
          </div>
          <p>{activeProfile.description}</p>
        </section>

        <section class="tuning-grid" id="tuning-sampling">
          <div class="control-matrix">
            <div class="panel-header inline"><span>Sampling</span><code>probability</code></div>
            {#each [
              ["Temperature", "temperature", 0, 2, 0.01],
              ["Top P", "topP", 0, 1, 0.01],
              ["Top K", "topK", 0, 200, 1],
              ["Min P", "minP", 0, 1, 0.01],
              ["Typical P", "typicalP", 0, 1, 0.01],
            ] as control}
              <label class="tuning-control">
                <span>{control[0]}</span>
                <input
                  type="range"
                  min={control[2]}
                  max={control[3]}
                  step={control[4]}
                  bind:value={sampling[control[1] as keyof typeof sampling]}
                />
                <input class="number-input" type="number" step={control[4]} bind:value={sampling[control[1] as keyof typeof sampling]} />
              </label>
            {/each}
          </div>

          <div class="distribution-panel" id="tuning-penalties">
            <div class="panel-header inline">
              <span>Penalties</span>
              <code>repetition control</code>
            </div>
            <div class="control-matrix compact">
              {#each [
                ["Repeat Penalty", "repeatPenalty", 0.8, 2, 0.01],
                ["Repeat Last N", "repeatLastN", -1, 4096, 1],
                ["Presence Penalty", "presencePenalty", -2, 2, 0.01],
                ["Frequency Penalty", "frequencyPenalty", -2, 2, 0.01],
              ] as control}
                <label class="tuning-control">
                  <span>{control[0]}</span>
                  <input
                    type="range"
                    min={control[2]}
                    max={control[3]}
                    step={control[4]}
                    bind:value={sampling[control[1] as keyof typeof sampling]}
                  />
                  <input class="number-input" type="number" step={control[4]} bind:value={sampling[control[1] as keyof typeof sampling]} />
                </label>
              {/each}
            </div>
            <div class="distribution-chart">
              {#each [88, 64, 42, 31, 24, 18, 13, 9, 7, 4] as value, index}
                <span style={`height: ${value}%`} title={`rank ${index + 1}`}></span>
              {/each}
            </div>
          </div>
        </section>

        <section class="runtime-panel" id="tuning-runtime">
          <div class="panel-header inline"><span>Runtime</span><code>allocation</code></div>
          <div class="runtime-grid">
            <label>
              <span>Backend</span>
              <select bind:value={sampling.backend}>
                <option value="llama.cpp">llama.cpp</option>
                <option value="mistral.rs">mistral.rs</option>
              </select>
            </label>
            <label>
              <span>KV cache</span>
              <select bind:value={sampling.kvCacheQuantization} onchange={() => void updateLoadPlan()}>
                <option value="f16">f16</option>
                <option value="q8_0">q8_0</option>
                <option value="q4_0">q4_0</option>
                <option value="f32">f32</option>
              </select>
            </label>
            <label>
              <span>Batch</span>
              <input type="number" min="1" step="1" bind:value={sampling.batchSize} />
            </label>
            <label>
              <span>Micro batch</span>
              <input type="number" min="1" step="1" bind:value={sampling.microBatchSize} />
            </label>
            <label>
              <span>CPU threads</span>
              <input type="number" min="1" step="1" bind:value={sampling.cpuThreads} />
            </label>
            <label>
              <span>Mirostat</span>
              <select bind:value={sampling.mirostatMode}>
                <option value={0}>Off</option>
                <option value={1}>v1</option>
                <option value={2}>v2</option>
              </select>
            </label>
            <label class="toggle-line">
              <span>mmap</span>
              <input type="checkbox" bind:checked={sampling.useMmap} />
            </label>
            <label class="toggle-line">
              <span>mlock</span>
              <input type="checkbox" bind:checked={sampling.useMlock} />
            </label>
            <label class="toggle-line">
              <span>Flash Attention</span>
              <input type="checkbox" bind:checked={sampling.flashAttention} />
            </label>
          </div>
        </section>

        <section class="schema-editor" id="tuning-json">
          <div>
            <div class="panel-header inline"><span>Live profile JSON</span><code>{activeProfile.id}</code></div>
            <pre>{profilePreviewJson}</pre>
          </div>
          <div>
            <div class="panel-header inline"><span>Output constraints</span><code>{activeProfile.output.mode}</code></div>
            <pre>{JSON.stringify(activeProfile.output, null, 2)}</pre>
          </div>
        </section>
      {:else if activeView === "knowledge"}
        <section class="workspace-header">
          <div>
            <p class="eyebrow">RAG Knowledge Bases</p>
            <h1>{activeKnowledgeBase?.name ?? "Local retrieval pipeline"}</h1>
          </div>
          <div class="import-row">
            <input
              aria-label="Document path"
              placeholder="Paste absolute path to .md, .txt, or source file"
              bind:value={knowledgeImportPath}
              onkeydown={(event) => {
                if (event.key === "Enter") void importKnowledgePath();
              }}
            />
            <button class="primary-button" disabled={knowledgeBusy || !knowledgeImportPath.trim()} onclick={importKnowledgePath}>
              <FileText size={15} />
              Import
            </button>
          </div>
        </section>

        <section class="rag-workbench">
          <aside class="rag-doc-tree">
            <div class="panel-header inline">
              <span>Document Tree</span>
              <code>{knowledgeDocuments.length} files</code>
            </div>
            {#if knowledgeDocuments.length === 0}
              <div class="empty-state compact">
                <FolderOpen size={26} />
                <span>Paste a local text, Markdown, or source-file path above.</span>
              </div>
            {:else}
              <div class="document-list">
                {#each knowledgeDocuments as document}
                  <button
                    class:active={selectedKnowledgeDocumentId === document.id}
                    onclick={() => selectKnowledgeDocument(document.id)}
                  >
                    <strong>{document.name}</strong>
                    <span>{formatTokens(document.chunkCount)} chunks / {formatBytes(document.sizeBytes)}</span>
                    <code>{formatShortDate(document.importedAt)}</code>
                  </button>
                {/each}
              </div>
            {/if}
          </aside>
          <article class="chunk-browser">
            <div class="panel-header inline">
              <span>Chunk Browser</span>
              <code>{activeKnowledgeDocument ? `${formatTokens(activeKnowledgeDocument.chunkCount)} chunks` : "no document"}</code>
            </div>
            {#if !activeKnowledgeDocument}
              <div class="empty-state compact">
                <Database size={26} />
                <span>Select or import a document to inspect generated chunks.</span>
              </div>
            {:else if ragChunkRows.length === 0}
              <div class="empty-state compact">
                <Database size={26} />
                <span>{activeKnowledgeDocument.name} has no indexed chunks.</span>
              </div>
            {:else}
              <div class="chunk-list">
                {#each ragChunkRows as chunk}
                  <div class:active={chunk.score !== null} class="chunk-row">
                    <div>
                      <code>#{chunk.index + 1}</code>
                      <span>{chunk.range}</span>
                    </div>
                    <p>{chunk.snippet}</p>
                    <i><b style={`width: ${(chunk.score ?? 0) * 100}%`}></b></i>
                  </div>
                {/each}
              </div>
            {/if}
          </article>
          <aside class="retrieval-panel">
            <div class="panel-header inline">
              <span>Retrieval Test</span>
              <code>{activeKnowledgeBase?.chunkCount ?? 0} chunks</code>
            </div>
            <div class="retrieval-query">
              <input
                placeholder="Test retrieval query..."
                bind:value={retrievalQuery}
                onkeydown={(event) => {
                  if (event.key === "Enter") void runRetrievalTest();
                }}
              />
              <button class="tool-button" disabled={knowledgeBusy || !retrievalQuery.trim()} onclick={runRetrievalTest}>Run</button>
            </div>
            <div class="rag-metric-grid">
              <div><span>Target</span><strong>1,200 chars</strong></div>
              <div><span>Overlap</span><strong>160 chars</strong></div>
              <div><span>Ranker</span><strong>Lexical cosine-lite</strong></div>
            </div>
            {#if retrievalResults.length === 0}
              <div class="empty-state compact">
                <Search size={26} />
                <span>Run a query to inspect the top 5 ranked chunks with similarity scores.</span>
              </div>
            {:else}
              <div class="retrieval-results">
                {#each retrievalResults.slice(0, 5) as result}
                  <article>
                    <div class="panel-header inline">
                      <span>{result.documentName} / #{result.chunkIndex + 1}</span>
                      <code>{formatNumber(result.score * 100, 0)}%</code>
                    </div>
                    <i><b style={`width: ${result.score * 100}%`}></b></i>
                    <p>{result.snippet}</p>
                  </article>
                {/each}
              </div>
            {/if}
          </aside>
        </section>
      {:else if activeView === "agents"}
        <section class="workspace-header">
          <div>
            <p class="eyebrow">Agents & Tools</p>
            <h1>Controlled local autonomy</h1>
          </div>
          <button class="primary-button">
            <BrainCircuit size={15} />
            New agent
          </button>
        </section>

        <section class="agent-workbench">
          <aside class="agent-list">
            <div class="panel-header inline"><span>Agents</span><code>1 draft</code></div>
            <button class="agent-row active">
              <BrainCircuit size={16} />
              <span>
                <strong>Analyst Agent</strong>
                <small>local model / approval-gated tools</small>
              </span>
              <code>draft</code>
            </button>
          </aside>
          <article class="agent-detail" id="agent-contract">
            <div class="panel-header inline"><span>System Contract</span><code>{activeProfile.id}</code></div>
            <label>
              <span>Name</span>
              <input value="Analyst Agent" aria-label="Agent name" />
            </label>
            <label>
              <span>System prompt</span>
              <textarea aria-label="Agent system prompt">{activeProfile.systemPrompt}</textarea>
            </label>
            <pre>{JSON.stringify({ model: selectedModel?.name ?? "unloaded", profile: activeProfile.id, knowledgeBase: activeKnowledgeBase?.name ?? "none", approvals: "required" }, null, 2)}</pre>
          </article>
          <aside class="tool-schema-list" id="agent-tools">
            <div class="panel-header inline"><span>Tools</span><code>{agentTools.length} available</code></div>
            {#each agentTools as tool}
              <label class:danger={tool.danger} class="tool-schema-row">
                <span>
                  <strong>{tool.name}</strong>
                  <small>{tool.danger ? "Explicit approval required" : "User controlled"}</small>
                </span>
                <input type="checkbox" checked={tool.enabled} />
              </label>
              <pre>{JSON.stringify({ name: tool.name, enabled: tool.enabled, risk: tool.danger ? "high" : "normal" }, null, 2)}</pre>
            {/each}
          </aside>
        </section>
      {:else if activeView === "api"}
        <section class="workspace-header">
          <div>
            <p class="eyebrow">Local API Server</p>
            <h1>OpenAI-compatible gateway</h1>
          </div>
          <button
            class:online={apiStatus?.enabled}
            class="power-button"
            disabled={engineBusy}
            onclick={() => (apiStatus?.enabled ? stopEngine() : startSelectedModel())}
          >
            <Power size={16} />
            {apiStatus?.enabled ? "Server on" : "Server off"}
          </button>
        </section>

        <section class:online={apiStatus?.enabled} class="api-dashboard">
          <div class="api-status-strip">
            <span>{apiStatus?.enabled ? "Running" : "Stopped"}</span>
            <code>{configuredBaseUrl}</code>
            <button aria-label="Copy base URL" onclick={copyBaseUrl}>
              <Clipboard size={15} />
              <small>{apiCopyStatus}</small>
            </button>
          </div>
          <div class="api-config-grid">
            <label>
              <span>Host</span>
              <input
                aria-label="API host"
                disabled={apiConfigBusy || apiEndpointLocked}
                bind:value={apiSettings.host}
              />
            </label>
            <label>
              <span>Port</span>
              <input
                aria-label="API port"
                type="number"
                min="1"
                max="65535"
                step="1"
                disabled={apiConfigBusy || apiEndpointLocked}
                bind:value={apiSettings.port}
              />
            </label>
            <button class="primary-button" disabled={apiConfigBusy || apiEndpointLocked} onclick={saveCurrentApiSettings}>
              <Server size={15} />
              {apiConfigBusy ? "Saving" : "Save endpoint"}
            </button>
          </div>
          {#if apiEndpointLocked}
            <p class="muted-copy">Stop the running local API server before changing host or port.</p>
          {/if}
          <div class="api-split">
            <div class="endpoint-table">
              <div class="panel-header inline"><span>Endpoints</span><code>{apiStatus?.endpoints?.length ?? 0} routes</code></div>
              {#each apiStatus?.endpoints ?? [] as endpoint}
                <div class="endpoint-row">
                  <code>{endpoint.method}</code>
                  <span>{endpoint.path}</span>
                  <small>{endpoint.description}</small>
                  <em>{endpoint.status}</em>
                </div>
              {/each}
            </div>
            <div class="api-example">
              <div class="panel-header inline"><span>cURL Example</span><code>stream</code></div>
              <pre class="curl-block">{`curl ${configuredBaseUrl}/chat/completions \\
  -H "Content-Type: application/json" \\
  -d '{
    "model": "${engineStatus?.activeModelName ?? "local-model"}",
    "messages": [{ "role": "user", "content": "ping" }],
    "stream": true
  }'`}</pre>
            </div>
          </div>
        </section>
      {:else if activeView === "benchmarks"}
        <section class="workspace-header">
          <div>
            <p class="eyebrow">Benchmarks</p>
            <h1>Throughput and load profile</h1>
          </div>
          <button class="primary-button" disabled={benchmarkBusy || !engineOnline} onclick={runModelBenchmark}>
            <Gauge size={15} />
            {benchmarkBusy ? "Benchmarking" : "Run benchmark"}
          </button>
        </section>

        <section class="benchmark-metrics">
          <article>
            <span>Gen eval/s</span>
            <strong>{formatNumber(benchmarks[0]?.tokensPerSecond ?? 0)}</strong>
          </article>
          <article>
            <span>Total tokens</span>
            <strong>{formatTokens(benchmarks[0]?.evalCount ?? 0)}</strong>
          </article>
          <article>
            <span>Load time</span>
            <strong>{formatNumber((benchmarks[0]?.loadDurationMs ?? 0) / 1000)}s</strong>
          </article>
          <article>
            <span>Runs</span>
            <strong>{formatTokens(benchmarks.length)}</strong>
          </article>
        </section>

        {#if benchmarks.length === 0}
          <section class="empty-state">
            <Gauge size={44} />
            <strong>No benchmark runs yet</strong>
            <span>Run a tokens/sec benchmark after loading a local model.</span>
          </section>
        {:else}
          <section class="benchmark-bars">
            <div class="benchmark-row benchmark-head">
              <span>Model</span>
              <span>Throughput</span>
              <span>Backend</span>
              <span>Tokens</span>
              <span>Load</span>
            </div>
            {#each benchmarks as result}
              <div class="benchmark-row">
                <span>{result.model}</span>
                <i style={`width: ${benchmarkBarWidth(result.tokensPerSecond)}%`} title={`${formatNumber(result.tokensPerSecond)} tok/s`}></i>
                <code>{result.backend}</code>
                <code>{formatTokens(result.evalCount)} eval</code>
                <code>{formatNumber(result.loadDurationMs / 1000)}s</code>
              </div>
            {/each}
          </section>
        {/if}
      {:else if activeView === "logs"}
        <section class="workspace-header">
          <div>
            <p class="eyebrow">System Logs</p>
            <h1>Runtime event stream</h1>
          </div>
          <div class="log-summary">
            <code>{formatTokens(filteredLogs.length)} visible</code>
            <code>{formatTokens(logs.length)} total</code>
          </div>
        </section>

        <section class="log-workbench">
          <aside class="log-level-rail" aria-label="Log level filters">
            {#each ["ALL", "INFO", "WARN", "ERROR", "DEBUG"] as level}
              <button class:active={logFilter === level} onclick={() => (logFilter = level)}>
                <span>{level}</span>
                <code>
                  {level === "ALL"
                    ? logs.length
                    : logs.filter((entry) => entry.level.toUpperCase() === level).length}
                </code>
              </button>
            {/each}
          </aside>

          <section class="log-terminal">
            <div class="log-terminal-head">
              <span>time</span>
              <span>level</span>
              <span>source</span>
              <span>message</span>
            </div>
            <div class="log-console">
              {#if filteredLogs.length === 0}
                <div class="log-empty">
                  <Terminal size={24} />
                  <span>No log lines match the current filter.</span>
                </div>
              {:else}
                {#each filteredLogs as entry}
                  <div
                    class:warn={entry.level === "WARN"}
                    class:error={entry.level === "ERROR"}
                    class:debug={entry.level === "DEBUG"}
                    class="log-line"
                  >
                    <code>{entry.timestamp}</code>
                    <strong>{entry.level}</strong>
                    <span>{entry.source}</span>
                    <p>{entry.message}</p>
                  </div>
                {/each}
              {/if}
            </div>
            <div class="log-command-bar">
              <code>grep</code>
              <input aria-label="Search logs" placeholder='grep "model_load"' bind:value={logSearch} />
              <button class="tool-button" disabled={!logSearch} onclick={() => (logSearch = "")}>Clear</button>
            </div>
          </section>
        </section>
      {:else}
        <section class="workspace-header">
          <div>
            <p class="eyebrow">Settings</p>
            <h1>Application control plane</h1>
          </div>
        </section>
        <section class="settings-workspace">
          <article id="settings-general">
            <div class="panel-header inline"><span>General</span><code>runtime</code></div>
            <label>
              <span>Default profile</span>
              <select bind:value={selectedProfileId} onchange={(event) => selectProfile(event.currentTarget.value)}>
                {#each profiles as profile}
                  <option value={profile.id}>{profile.name}</option>
                {/each}
              </select>
            </label>
            <label>
              <span>Default backend</span>
              <select bind:value={sampling.backend}>
                <option value="llama.cpp">llama.cpp</option>
                <option value="mistral.rs">mistral.rs</option>
              </select>
            </label>
          </article>
          <article id="settings-appearance">
            <div class="panel-header inline"><span>Appearance</span><code>{theme}</code></div>
            <label class="toggle-line">
              <span>Light mode</span>
              <input type="checkbox" checked={theme === "light"} onchange={toggleTheme} />
            </label>
            <label class="toggle-line">
              <span>Collapse inspector</span>
              <input type="checkbox" bind:checked={rightCollapsed} />
            </label>
            <label class="toggle-line">
              <span>Collapse context</span>
              <input type="checkbox" bind:checked={leftCollapsed} />
            </label>
          </article>
          <article id="settings-storage">
            <div class="panel-header inline"><span>Storage Paths</span><code>local</code></div>
            <div class="path-row">
              <span>Model library</span>
              <code>./models</code>
            </div>
            <div class="path-row">
              <span>Profiles</span>
              <code>app-config/profiles/*.kivarro.json</code>
            </div>
            <div class="path-row">
              <span>RAG store</span>
              <code>app-config/knowledge-store.json</code>
            </div>
          </article>
          <article id="settings-advanced">
            <div class="panel-header inline"><span>Advanced</span><code>api</code></div>
            <label>
              <span>API host</span>
              <input disabled={apiEndpointLocked} bind:value={apiSettings.host} />
            </label>
            <label>
              <span>API port</span>
              <input type="number" min="1" max="65535" disabled={apiEndpointLocked} bind:value={apiSettings.port} />
            </label>
            <button class="primary-button" disabled={apiConfigBusy || apiEndpointLocked} onclick={saveCurrentApiSettings}>Save API endpoint</button>
          </article>
          <div class="settings-savebar">
            <span>{apiEndpointLocked ? "Stop the local API server before endpoint changes." : "Settings changes are ready to save."}</span>
            <button class="primary-button" disabled={apiConfigBusy || apiEndpointLocked} onclick={saveCurrentApiSettings}>
              <Clipboard size={15} />
              Save settings
            </button>
          </div>
        </section>
      {/if}
    </main>

    <aside class="inspector">
      <div class="panel-header">
        <span>Inspector</span>
        <span class:status-online={metrics?.apiOnline} class:status-idle={!metrics?.apiOnline}>
          <Circle size={10} />
        </span>
      </div>

      <section class="inspector-section">
        <div class="section-label">Active Profile</div>
        <div class="stat-row">
          <span>{activeProfile.name}</span>
          <code>{profileSaveStatus}</code>
        </div>
        <button class="inspector-action" onclick={saveCurrentProfile}>Save profile</button>
      </section>

      <section class="inspector-section">
        <div class="section-label">Engine</div>
        <div class="stat-row">
          <span>Status</span>
          <code>{engineStatus?.state ?? "offline"}</code>
        </div>
        <div class="stat-row">
          <span>Runtime</span>
          <code>{engineStatus?.backend ?? "llama.cpp"}</code>
        </div>
        <div class="stat-row">
          <span>PID</span>
          <code>{engineStatus?.pid ?? "none"}</code>
        </div>
        <div class="stat-row">
          <span>Binary</span>
          <code>{engineStatus?.binaryPath ?? "unconfigured"}</code>
        </div>
        <p class="engine-message">{engineNotice}</p>
        <div class="engine-actions">
          <button class="inspector-action" disabled={engineBusy} onclick={startSelectedModel}>
            {engineOnline ? "Restart model" : "Load model"}
          </button>
          <button class="inspector-action secondary" disabled={engineBusy || !apiStatus?.enabled} onclick={stopEngine}>
            Stop
          </button>
        </div>
      </section>

      <section class="inspector-section">
        <div class="section-label">Selected Model</div>
        {#if selectedModel}
          <div class="stat-row">
            <span>Name</span>
            <code>{selectedModel.name}</code>
          </div>
          <div class="stat-row">
            <span>Arch</span>
            <code>{optionalText(selectedModel.architecture)}</code>
          </div>
          <div class="stat-row">
            <span>Quant</span>
            <code>{optionalText(selectedModel.quantization, selectedModel.format)}</code>
          </div>
          <div class="stat-row">
            <span>Context</span>
            <code>{formatTokenLimit(selectedModel.contextLength)}</code>
          </div>
          <div class="stat-row">
            <span>Layers</span>
            <code>{formatLayerCount(selectedModel.blockCount)}</code>
          </div>
          <div class="stat-row">
            <span>Source</span>
            <code>{selectedModel.metadataSource}</code>
          </div>
        {:else}
          <p class="muted-copy">No model selected.</p>
        {/if}
      </section>

      <section class="inspector-section">
        <div class="section-label">Run Parameters</div>
        <label>
          Temperature
          <input type="number" min="0" max="2" step="0.01" bind:value={sampling.temperature} />
        </label>
        <label>
          Top P
          <input type="number" min="0" max="1" step="0.01" bind:value={sampling.topP} />
        </label>
        <label>
          Max tokens
          <input type="number" min="1" max="65536" step="1" bind:value={sampling.maxTokens} />
        </label>
        <label>
          KV cache
          <select bind:value={sampling.kvCacheQuantization} onchange={() => void updateLoadPlan()}>
            <option value="f16">f16</option>
            <option value="q8_0">q8_0</option>
            <option value="q4_0">q4_0</option>
            <option value="f32">f32</option>
          </select>
        </label>
      </section>

      <section class="inspector-section">
        <div class="section-label">Context Window</div>
        <div class="context-readout">
          <strong>{formatTokens(metrics?.contextUsedTokens ?? 0)}</strong>
          <span>/ {formatTokens(metrics?.contextTotalTokens ?? sampling.contextLength)} tokens</span>
        </div>
        <div class="mini-bar"><span style={`width: ${contextPercent}%`}></span></div>
      </section>

      <section class="inspector-section">
        <div class="section-label">Hardware</div>
        <div class="stat-row">
          <span>CPU</span>
          <code>{formatNumber(metrics?.cpuUtilizationPercent ?? 0)}%</code>
        </div>
        <div class="stat-row">
          <span>GPU</span>
          <code>{formatNumber(metrics?.gpuUtilizationPercent ?? 0)}%</code>
        </div>
        <div class="stat-row">
          <span>RAM</span>
          <code>{formatNumber(metrics?.ramUsedGib ?? 0)} / {formatNumber(metrics?.ramTotalGib ?? 0)} GiB</code>
        </div>
        {#if loadPlan}
          <div class="stat-row">
            <span>Load plan</span>
            <code>{formatNumber(loadPlan.totalRequiredGib)} GiB</code>
          </div>
        {/if}
      </section>
    </aside>
  </div>

  <footer class="statusbar">
    <span>{metrics?.activeModel ?? "No model loaded"}</span>
    <span>GPU {formatNumber(metrics?.gpuUtilizationPercent ?? 0)}%</span>
    <span class="status-meter"><i style={`width: ${ramPercent}%`}></i>RAM {formatNumber(metrics?.ramUsedGib ?? 0)} / {formatNumber(metrics?.ramTotalGib ?? 0)} GiB</span>
    <span>{formatNumber(metrics?.tokensPerSecond ?? 0)} tok/s</span>
    <span class:online={metrics?.apiOnline}>API :{metrics?.apiPort ?? 8080}</span>
    <code>{metrics?.serverUrl ?? "http://127.0.0.1:8080/v1"}</code>
  </footer>

  {#if commandPaletteOpen}
    <button class="palette-backdrop" aria-label="Close command palette" onclick={closeCommandPalette}></button>
    <section class="command-palette" aria-label="Command palette">
      <div class="palette-input">
        <Search size={16} />
        <input
          aria-label="Command palette search"
          placeholder="Search commands, views, and runtime actions"
          bind:this={paletteInput}
          bind:value={paletteQuery}
          onkeydown={(event) => {
            if (event.key === "Escape") {
              event.preventDefault();
              closeCommandPalette();
            }
            if (event.key === "Enter") {
              event.preventDefault();
              void runPaletteAction(filteredPaletteActions.find((action) => !action.disabled));
            }
          }}
        />
      </div>
      <div class="palette-results">
        {#if filteredPaletteActions.length === 0}
          <div class="palette-empty">[ no matching actions ]</div>
        {:else}
          {#each filteredPaletteActions as action}
            <button disabled={action.disabled} onclick={() => void runPaletteAction(action)}>
              <svelte:component this={action.icon} size={15} />
              <span>
                <strong>{action.label}</strong>
                <small>{action.detail}</small>
              </span>
              {#if action.disabled}
                <code>disabled</code>
              {/if}
            </button>
          {/each}
        {/if}
      </div>
    </section>
  {/if}
</div>

<style>
  .app {
    width: 100vw;
    height: 100vh;
    display: grid;
    grid-template-rows: 36px minmax(0, 1fr) 24px;
    background: var(--bg);
    color: var(--text);
  }

  .titlebar {
    display: grid;
    grid-template-columns: 170px minmax(0, 1fr) 220px;
    align-items: center;
    border-bottom: 1px solid var(--border);
    background: color-mix(in srgb, var(--panel) 92%, transparent);
    user-select: none;
  }

  .window-controls,
  .quick-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
  }

  .quick-actions {
    justify-content: flex-end;
  }

  .window-control,
  .icon-button,
  .rail-button,
  .tool-button,
  .primary-button,
  .send-button,
  .power-button {
    border: 1px solid transparent;
    background: transparent;
    cursor: pointer;
  }

  .window-control {
    width: 14px;
    height: 14px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    color: rgba(14, 14, 17, 0.72);
  }

  .window-control.close {
    background: #ff5f57;
  }

  .window-control.minimize {
    background: #ffbd2e;
  }

  .window-control.maximize {
    background: #28c840;
  }

  .title-identity {
    display: flex;
    justify-content: center;
    align-items: center;
    gap: 10px;
    min-width: 0;
    font-size: 12px;
  }

  .wordmark {
    color: var(--amber);
    font-weight: 800;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .icon-button {
    width: 28px;
    height: 26px;
    display: grid;
    place-items: center;
    border-radius: 6px;
    color: var(--muted);
  }

  .icon-button:hover,
  .rail-button:hover,
  .tool-button:hover {
    background: var(--panel-2);
    color: var(--text);
  }

  button:disabled {
    cursor: not-allowed;
    opacity: 0.55;
  }

  .shell {
    min-height: 0;
    display: grid;
    grid-template-columns: 64px 280px minmax(0, 1fr) 320px;
    transition: grid-template-columns 160ms ease;
  }

  .shell.left-collapsed {
    grid-template-columns: 64px 0 minmax(0, 1fr) 320px;
  }

  .shell.right-collapsed {
    grid-template-columns: 64px 280px minmax(0, 1fr) 0;
  }

  .shell.left-collapsed.right-collapsed {
    grid-template-columns: 64px 0 minmax(0, 1fr) 0;
  }

  .nav-rail,
  .context-panel,
  .inspector {
    min-height: 0;
    border-right: 1px solid var(--border);
    background: var(--panel);
  }

  .nav-rail {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: space-between;
    padding: 8px 0;
  }

  .rail-stack {
    display: grid;
    gap: 4px;
  }

  .rail-button {
    width: 40px;
    height: 36px;
    display: grid;
    place-items: center;
    border-radius: 7px;
    color: var(--muted);
  }

  .rail-button.active {
    color: #0e0e11;
    background: var(--amber);
  }

  .rail-button.monitor {
    color: var(--cyan);
  }

  .context-panel,
  .inspector {
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .shell.left-collapsed .context-panel,
  .shell.right-collapsed .inspector {
    border: 0;
    pointer-events: none;
  }

  .panel-header {
    height: 44px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex: 0 0 auto;
    padding: 0 14px;
    border-bottom: 1px solid var(--border);
    color: var(--text);
    font-size: 13px;
    font-weight: 700;
  }

  .panel-header.inline {
    height: auto;
    padding: 0;
    border: 0;
    margin-bottom: 12px;
  }

  .context-panel > :not(.panel-header),
  .inspector-section {
    margin: 12px;
  }

  .profile-select,
  .search-box,
  .model-import-box,
  .metric-stack,
  .profile-context-list,
  .jump-list,
  .inspector-section {
    display: grid;
    gap: 8px;
  }

  label,
  .section-label {
    color: var(--muted);
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
  }

  select,
  input,
  textarea {
    width: 100%;
    border: 1px solid var(--border);
    border-radius: 7px;
    color: var(--text);
    background: var(--panel-2);
  }

  select,
  input {
    height: 34px;
    padding: 0 10px;
  }

  textarea {
    min-height: 58px;
    max-height: 140px;
    resize: vertical;
    padding: 10px 12px;
  }

  .search-box {
    grid-template-columns: 18px minmax(0, 1fr);
    align-items: center;
    padding: 0 10px;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--panel-2);
  }

  .search-box input {
    border: 0;
    padding: 0;
    background: transparent;
  }

  .history-group {
    display: grid;
    gap: 4px;
    margin-top: 10px;
  }

  .history-title {
    color: var(--dim);
    font-size: 11px;
    font-family: var(--mono);
  }

  .history-item,
  .model-mini,
  .profile-context-card,
  .jump-list button {
    width: 100%;
    border: 1px solid transparent;
    border-radius: 7px;
    background: transparent;
    color: var(--muted);
    text-align: left;
    cursor: pointer;
  }

  .history-item {
    min-height: 32px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 8px;
  }

  .history-item span {
    min-width: 0;
    flex: 1;
  }

  .history-item:hover,
  .model-mini:hover,
  .profile-context-card:hover,
  .jump-list button:hover {
    color: var(--text);
    background: var(--panel-2);
  }

  .history-item.active,
  .profile-context-card.active {
    color: var(--text);
    border-color: color-mix(in srgb, var(--cyan) 36%, var(--border));
    background: color-mix(in srgb, var(--cyan) 9%, var(--panel-2));
  }

  .kb-create-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 36px;
    gap: 6px;
  }

  .model-import-box {
    display: grid;
    gap: 6px;
    margin-top: 12px;
  }

  .model-import-box > div {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 34px;
    gap: 6px;
  }

  .model-import-box button {
    display: grid;
    place-items: center;
    border: 1px solid var(--border);
    border-radius: 7px;
    color: var(--text);
    background: var(--panel-2);
    cursor: pointer;
  }

  .model-import-box small {
    color: var(--dim);
    font-size: 11px;
    line-height: 1.35;
  }

  .model-mini {
    display: grid;
    gap: 4px;
    padding: 8px;
  }

  .profile-context-card {
    display: grid;
    gap: 5px;
    padding: 9px;
  }

  .profile-context-card span {
    overflow: hidden;
    color: var(--text);
    font-size: 12px;
    font-weight: 800;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .profile-context-card small {
    display: -webkit-box;
    overflow: hidden;
    color: var(--dim);
    font-size: 11px;
    line-height: 1.35;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
  }

  .profile-context-card code,
  .context-stat code {
    overflow: hidden;
    color: var(--cyan);
    font-size: 11px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .jump-list button {
    min-height: 30px;
    padding: 0 9px;
    font-family: var(--mono);
    font-size: 11px;
    letter-spacing: 0.04em;
    text-align: left;
    text-transform: uppercase;
  }

  .context-stat {
    display: grid;
    gap: 5px;
    padding: 9px;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--panel-2);
  }

  .context-stat span {
    color: var(--dim);
    font-size: 11px;
    font-weight: 800;
    text-transform: uppercase;
  }

  .model-mini.active,
  .model-row.active {
    border-color: color-mix(in srgb, var(--cyan) 42%, var(--border));
    background: color-mix(in srgb, var(--cyan) 10%, var(--panel-2));
    color: var(--text);
  }

  .model-mini small,
  .muted-copy,
  .profile-description {
    color: var(--dim);
    font-size: 12px;
  }

  .profile-description {
    margin: 0;
    line-height: 1.45;
  }

  .metric-stack > div {
    display: grid;
    gap: 4px;
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--panel-2);
  }

  .filter-row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .filter-row button,
  .segmented button {
    height: 28px;
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--muted);
    background: var(--panel-2);
    cursor: pointer;
  }

  .filter-row button.active,
  .segmented button.active {
    color: #0e0e11;
    border-color: var(--amber);
    background: var(--amber);
  }

  .workspace {
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 14px;
    overflow: auto;
    padding: 18px;
    background:
      linear-gradient(rgba(255, 255, 255, 0.025) 1px, transparent 1px),
      linear-gradient(90deg, rgba(255, 255, 255, 0.025) 1px, transparent 1px),
      var(--bg);
    background-size: 28px 28px;
  }

  .workspace-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    flex: 0 0 auto;
  }

  .workspace-header h1 {
    margin: 2px 0 0;
    font-size: 22px;
    line-height: 1.1;
    letter-spacing: 0;
  }

  .eyebrow {
    margin: 0;
    color: var(--amber);
    font-family: var(--mono);
    font-size: 11px;
    text-transform: uppercase;
  }

  .header-actions,
  .segmented {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .tool-button,
  .primary-button,
  .power-button {
    height: 34px;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
    border-radius: 7px;
    border-color: var(--border);
    background: var(--panel);
    color: var(--text);
  }

  .tool-button.active,
  .primary-button {
    color: #0e0e11;
    border-color: var(--amber);
    background: var(--amber);
  }

  .primary-button.online {
    border-color: var(--cyan);
    background: var(--cyan);
  }

  .power-button {
    color: var(--red);
  }

  .power-button.online {
    color: var(--cyan);
  }

  .chat-surface {
    min-height: 0;
    flex: 1 1 auto;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 12px;
  }

  .chat-surface.split {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .chat-pane,
  .model-table,
  .load-plan,
  .control-band,
  .hardware-plan,
  .profile-strip,
  .runtime-panel,
  .distribution-panel,
  .schema-editor > div,
  .api-dashboard,
  .log-console {
    border: 1px solid var(--border);
    border-radius: 8px;
    background: color-mix(in srgb, var(--panel) 94%, transparent);
  }

  .chat-pane {
    min-height: 420px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .pane-header {
    height: 38px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 12px;
    border-bottom: 1px solid var(--border);
  }

  code,
  pre,
  .statusbar,
  .log-console {
    font-family: var(--mono);
  }

  .message-list {
    min-height: 0;
    overflow: auto;
    padding: 14px;
  }

  .message {
    display: grid;
    gap: 8px;
    max-width: 860px;
    padding: 12px 0;
    border-bottom: 1px solid var(--border);
  }

  .message.system {
    color: var(--muted);
  }

  .message.streaming {
    color: var(--text);
  }

  .message-meta {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    color: var(--dim);
    font-size: 11px;
  }

  .message p {
    margin: 0;
    color: var(--text);
    font-size: 14px;
    line-height: 1.65;
  }

  .stream-cursor {
    display: inline-block;
    width: 7px;
    height: 14px;
    margin-left: 3px;
    vertical-align: -2px;
    background: var(--cyan);
    animation: blink 1s steps(2, start) infinite;
  }

  @keyframes blink {
    50% {
      opacity: 0;
    }
  }

  .prompt-dock {
    flex: 0 0 auto;
    display: grid;
    gap: 8px;
  }

  .context-meter,
  .mini-bar,
  .status-meter {
    position: relative;
    overflow: hidden;
    border: 1px solid var(--border);
    background: var(--panel-2);
  }

  .context-meter {
    height: 8px;
    border-radius: var(--radius-sm);
  }

  .context-meter span,
  .mini-bar span,
  .status-meter i {
    display: block;
    height: 100%;
    background: linear-gradient(90deg, var(--cyan), var(--amber), var(--red));
  }

  .marker {
    position: absolute;
    top: -2px;
    width: 1px;
    height: 12px;
    background: var(--border-strong);
  }

  .marker-4k {
    left: 12.5%;
  }

  .marker-8k {
    left: 25%;
  }

  .marker-16k {
    left: 50%;
  }

  .marker-32k {
    left: 100%;
  }

  .prompt-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 46px;
    gap: 8px;
  }

  .send-button {
    display: grid;
    place-items: center;
    border-radius: 7px;
    color: #0e0e11;
    background: var(--amber);
  }

  .send-button.stopping {
    background: var(--red);
  }

  .send-button:disabled {
    cursor: wait;
    opacity: 0.72;
  }

  .empty-state {
    min-height: 360px;
    display: grid;
    place-items: center;
    align-content: center;
    gap: 10px;
    border: 1px dashed var(--border-strong);
    border-radius: 8px;
    color: var(--muted);
  }

  .empty-state strong {
    color: var(--text);
    font-size: 16px;
  }

  .empty-state.compact {
    min-height: 220px;
    padding: 18px;
    text-align: center;
  }

  .model-table {
    overflow: auto;
  }

  .table-row {
    display: grid;
    grid-template-columns: minmax(180px, 2fr) 92px 96px 110px 88px 96px 104px;
    gap: 12px;
    align-items: center;
    min-height: 42px;
    min-width: 0;
    padding: 0 12px;
    border-bottom: 1px solid var(--border);
    font-size: 13px;
  }

  .table-row > *,
  .stat-row > * {
    min-width: 0;
  }

  .table-row code,
  .stat-row code {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .model-name-cell {
    display: grid;
    gap: 2px;
    min-width: 0;
  }

  .model-name-cell strong,
  .model-name-cell small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .model-name-cell small {
    color: var(--dim);
    font-family: var(--mono);
    font-size: 11px;
  }

  .model-row {
    width: 100%;
    border-left: 0;
    border-right: 0;
    border-top: 0;
    color: var(--text);
    text-align: left;
    background: transparent;
    cursor: pointer;
  }

  .model-row:hover {
    background: var(--panel-2);
  }

  .table-row.header {
    color: var(--dim);
    font-size: 11px;
    font-weight: 800;
    text-transform: uppercase;
  }

  .good {
    color: var(--green);
  }

  .warn {
    color: var(--amber);
  }

  .blueprint-grid,
  .rag-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 12px;
  }

  .block-top,
  .stat-row,
  .context-readout {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .memory-bar {
    height: 12px;
    overflow: hidden;
    border-radius: 999px;
    background: var(--panel-2);
  }

  .memory-bar span {
    display: block;
    height: 100%;
    background: var(--amber);
  }

  .control-band {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 16px;
    padding: 14px;
  }

  .load-plan,
  .hardware-plan,
  .profile-strip,
  .runtime-panel {
    padding: 14px;
  }

  .load-plan p,
  .hardware-plan p,
  .profile-strip p {
    margin: 10px 0 0;
    color: var(--muted);
    font-size: 12px;
    line-height: 1.5;
  }

  .load-plan-grid,
  .allocation-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 10px;
  }

  .load-plan-grid div,
  .allocation-grid div,
  .profile-strip div {
    display: grid;
    gap: 4px;
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--panel-2);
  }

  .load-plan-grid span,
  .allocation-grid span,
  .profile-strip span {
    color: var(--dim);
    font-size: 11px;
    font-weight: 800;
    text-transform: uppercase;
  }

  .segment-bar {
    height: 12px;
    display: flex;
    overflow: hidden;
    margin-top: 12px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--panel-2);
  }

  .segment-bar.large {
    height: 16px;
  }

  .segment-bar span {
    min-width: 4px;
  }

  .segment-amber {
    background: var(--amber);
  }

  .segment-cyan {
    background: var(--cyan);
  }

  .segment-magenta {
    background: var(--magenta);
  }

  .segment-green {
    background: var(--green);
  }

  .runtime-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px;
    margin-top: 12px;
  }

  .runtime-grid label {
    display: grid;
    gap: 8px;
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--panel);
    text-transform: none;
  }

  .runtime-grid .toggle-line {
    grid-template-columns: minmax(0, 1fr) 24px;
    align-items: center;
  }

  input[type="checkbox"] {
    appearance: none;
    width: 18px;
    height: 18px;
    display: grid;
    place-items: center;
    padding: 0;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--bg-elevated);
    cursor: pointer;
  }

  input[type="checkbox"]::after {
    content: "";
    width: 8px;
    height: 8px;
    background: transparent;
  }

  input[type="checkbox"]:checked {
    border-color: var(--accent-primary);
    background: color-mix(in srgb, var(--accent-primary) 18%, var(--bg-elevated));
  }

  input[type="checkbox"]:checked::after {
    background: var(--accent-primary);
  }

  .control-band > div,
  .tuning-control {
    display: grid;
    gap: 8px;
  }

  input[type="range"] {
    appearance: none;
    height: 18px;
    padding: 0;
    background: transparent;
    cursor: pointer;
  }

  input[type="range"]::-webkit-slider-runnable-track {
    height: 2px;
    border: 1px solid var(--border-default);
    background: var(--bg-elevated);
  }

  input[type="range"]::-webkit-slider-thumb {
    appearance: none;
    width: 12px;
    height: 12px;
    margin-top: -6px;
    border: 1px solid color-mix(in srgb, var(--accent-primary) 60%, var(--border-strong));
    border-radius: var(--radius-sm);
    background: var(--accent-primary);
  }

  input[type="range"]::-moz-range-track {
    height: 2px;
    border: 1px solid var(--border-default);
    background: var(--bg-elevated);
  }

  input[type="range"]::-moz-range-thumb {
    width: 12px;
    height: 12px;
    border: 1px solid color-mix(in srgb, var(--accent-primary) 60%, var(--border-strong));
    border-radius: var(--radius-sm);
    background: var(--accent-primary);
  }

  .tuning-grid {
    display: grid;
    grid-template-columns: minmax(340px, 0.9fr) minmax(0, 1.1fr);
    gap: 12px;
  }

  .control-matrix {
    display: grid;
    gap: 10px;
  }

  .control-matrix.compact {
    gap: 8px;
  }

  .tuning-control {
    grid-template-columns: 130px minmax(0, 1fr) 88px;
    align-items: center;
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--panel);
  }

  .number-input {
    font-family: var(--mono);
  }

  .distribution-panel {
    display: grid;
    align-content: start;
    gap: 12px;
    padding: 14px;
  }

  .distribution-chart {
    height: 148px;
    display: flex;
    align-items: end;
    gap: 8px;
    padding: 12px;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--panel-2);
  }

  .distribution-chart span {
    width: 100%;
    min-height: 4px;
    border-radius: 5px 5px 0 0;
    background: linear-gradient(180deg, var(--cyan), var(--amber));
  }

  .schema-editor {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
  }

  .schema-editor > div {
    min-width: 0;
    padding: 14px;
  }

  pre {
    min-height: 180px;
    overflow: auto;
    margin: 0;
    padding: 12px;
    border: 1px solid var(--border);
    border-radius: 7px;
    color: var(--cyan);
    background: #0b0b0d;
    font-size: 12px;
    line-height: 1.6;
  }

  .rag-workbench > * {
    min-height: 300px;
    padding: 14px;
  }

  .import-row {
    min-width: min(560px, 100%);
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 8px;
  }

  .document-list,
  .retrieval-results {
    display: grid;
    gap: 8px;
  }

  .document-list button,
  .retrieval-results article,
  .rag-metric-grid div {
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--panel-2);
  }

  .document-list strong,
  .document-list span,
  .rag-metric-grid span,
  .rag-metric-grid strong {
    display: block;
  }

  .document-list span,
  .rag-metric-grid span,
  .retrieval-results p {
    color: var(--muted);
    font-size: 12px;
  }

  .rag-metric-grid {
    display: grid;
    gap: 8px;
  }

  .retrieval-results article {
    display: grid;
    gap: 8px;
  }

  .retrieval-results p {
    margin: 0;
    line-height: 1.55;
  }

  .retrieval-query,
  .api-url {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 8px;
    align-items: center;
    padding: 12px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--panel);
  }

  .agent-canvas {
    min-height: 300px;
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 16px;
    align-items: center;
  }

  .api-dashboard {
    display: grid;
    gap: 12px;
    padding: 14px;
  }

  .api-config-grid {
    display: grid;
    grid-template-columns: minmax(0, 1.2fr) minmax(120px, 0.45fr) auto;
    gap: 10px;
    align-items: end;
  }

  .api-config-grid label {
    display: grid;
    gap: 6px;
  }

  .api-url {
    grid-template-columns: 90px minmax(0, 1fr) 92px;
  }

  .endpoint-table {
    display: grid;
    gap: 8px;
  }

  .endpoint-row {
    display: grid;
    grid-template-columns: 70px minmax(180px, 0.8fr) minmax(0, 1fr) 90px;
    gap: 10px;
    align-items: center;
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--panel-2);
  }

  .endpoint-row small {
    color: var(--muted);
  }

  .endpoint-row em {
    color: var(--amber);
    font-style: normal;
  }

  .benchmark-bars {
    display: grid;
    gap: 10px;
  }

  .benchmark-bars div {
    display: grid;
    grid-template-columns: 180px minmax(0, 1fr) 100px;
    align-items: center;
    gap: 10px;
  }

  .benchmark-bars i {
    height: 14px;
    border-radius: 999px;
    background: var(--cyan);
  }

  .log-console {
    flex: 1 1 auto;
    overflow: auto;
    padding: 12px;
    background: #09090b;
  }

  .log-line {
    display: grid;
    grid-template-columns: 120px 56px 110px minmax(0, 1fr);
    gap: 10px;
    min-height: 28px;
    align-items: start;
    color: var(--muted);
    font-size: 12px;
  }

  .log-line strong {
    color: var(--green);
  }

  .log-line.warn strong {
    color: var(--amber);
  }

  .log-line.error strong {
    color: var(--red);
  }

  .log-line p {
    margin: 0;
  }

  .inspector {
    border-left: 1px solid var(--border);
    border-right: 0;
  }

  .inspector-section {
    padding-bottom: 12px;
    border-bottom: 1px solid var(--border);
  }

  .inspector-section label {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 86px;
    align-items: center;
    gap: 8px;
    margin-top: 8px;
    text-transform: none;
  }

  .inspector-action {
    height: 30px;
    border: 1px solid var(--border);
    border-radius: 6px;
    color: #0e0e11;
    background: var(--amber);
    cursor: pointer;
  }

  .inspector-action.secondary {
    color: var(--text);
    background: var(--panel-2);
  }

  .engine-message {
    margin: 8px 0 0;
    color: var(--dim);
    font-size: 12px;
    line-height: 1.45;
  }

  .engine-actions {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 76px;
    gap: 8px;
    margin-top: 10px;
  }

  .mini-bar {
    height: 8px;
    margin-top: 10px;
    border-radius: 999px;
  }

  .status-online {
    display: grid;
    place-items: center;
    color: var(--cyan);
    fill: var(--cyan);
  }

  .status-idle {
    display: grid;
    place-items: center;
    color: var(--dim);
    fill: var(--dim);
  }

  .statusbar {
    display: flex;
    align-items: center;
    gap: 14px;
    min-width: 0;
    padding: 0 10px;
    border-top: 1px solid var(--border);
    background: var(--panel);
    color: var(--muted);
    font-size: 11px;
    white-space: nowrap;
  }

  .statusbar span,
  .statusbar code {
    min-width: 0;
  }

  .statusbar .online {
    color: var(--cyan);
  }

  .status-meter {
    width: 190px;
    height: 14px;
    display: inline-flex;
    align-items: center;
    padding-left: 8px;
    border-radius: 999px;
  }

  .status-meter i {
    position: absolute;
    inset: 0 auto 0 0;
    opacity: 0.22;
  }

  .palette-backdrop {
    position: fixed;
    inset: 0;
    border: 0;
    background: color-mix(in srgb, var(--bg-app) 72%, transparent);
    backdrop-filter: blur(6px);
    cursor: default;
  }

  .command-palette {
    position: fixed;
    top: 64px;
    left: 50%;
    width: min(600px, calc(100vw - 48px));
    max-height: 400px;
    transform: translateX(-50%);
    overflow: hidden;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-md);
    background: var(--panel);
  }

  .palette-input {
    display: grid;
    grid-template-columns: 24px minmax(0, 1fr);
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    border-bottom: 1px solid var(--border);
  }

  .palette-input input {
    border: 0;
    background: transparent;
  }

  .palette-results {
    display: grid;
    max-height: 360px;
    overflow: auto;
    padding: 6px;
  }

  .palette-results button {
    min-height: 42px;
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
  }

  .palette-results button:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .palette-results button:hover {
    color: var(--text);
    background: var(--panel-2);
  }

  .palette-results button span {
    display: grid;
    min-width: 0;
    gap: 2px;
  }

  .palette-results button strong,
  .palette-results button small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .palette-results button strong {
    color: var(--text);
    font-size: 13px;
  }

  .palette-results button small,
  .palette-empty {
    color: var(--dim);
    font-size: 11px;
  }

  .palette-results button code {
    color: var(--dim);
    font-size: 10px;
    text-transform: uppercase;
  }

  .palette-empty {
    min-height: 56px;
    display: grid;
    place-items: center;
    font-family: var(--mono);
  }


  /* Precision Instrumentation & Tactile Calm layer */
  .app {
    min-width: 900px;
    min-height: 720px;
    grid-template-rows: 40px minmax(0, 1fr) 24px;
    background: var(--bg-app);
    color: var(--text-primary);
    font-size: var(--text-sm);
  }

  .titlebar {
    grid-template-columns: minmax(320px, 0.85fr) minmax(320px, 600px) minmax(300px, 0.85fr);
    height: 40px;
    border-bottom: 1px solid var(--border-default);
    background: var(--bg-app);
  }

  .window-controls,
  .quick-actions {
    gap: 4px;
    padding: 0;
  }

  .title-actions {
    min-width: 0;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 12px;
    padding: 0 10px 0 0;
  }

  .window-control {
    width: 28px;
    height: 24px;
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    background: transparent;
  }

  .window-control.close,
  .window-control.minimize,
  .window-control.maximize {
    background: transparent;
  }

  .window-control:hover {
    color: var(--text-primary);
    background: var(--bg-elevated);
  }

  .window-control.close:hover {
    color: var(--bg-app);
    background: var(--accent-danger);
  }

  .title-identity {
    min-width: 0;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    align-items: center;
    justify-content: flex-start;
    gap: 14px;
    padding: 0 12px;
    font-size: var(--text-xs);
  }

  .brand-lockup {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    color: var(--accent-primary);
  }

  .wordmark {
    color: var(--text-primary);
    font-family: var(--font-mono);
    font-size: 12px;
    font-weight: 800;
    letter-spacing: 0.2em;
  }

  .title-model {
    min-width: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    color: var(--text-secondary);
    font-family: var(--font-mono);
  }

  .title-model > span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .title-model code {
    color: var(--text-tertiary);
    font-size: 10px;
  }

  .title-context-meter {
    grid-column: 1 / -1;
    overflow: hidden;
    height: 2px;
    border-radius: var(--radius-sm);
    background: var(--bg-elevated);
  }

  .title-context-meter b {
    display: block;
    height: 100%;
    background: var(--accent-primary);
  }

  .title-command {
    justify-self: center;
    width: min(600px, 100%);
    height: 28px;
    display: grid;
    grid-template-columns: 18px minmax(0, 1fr) auto;
    align-items: center;
    gap: 7px;
    padding: 0 8px;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    color: var(--text-tertiary);
    background: var(--bg-elevated);
    cursor: pointer;
  }

  .title-command span {
    overflow: hidden;
    text-align: left;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .title-command code {
    padding: 1px 5px;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .icon-button {
    width: 28px;
    height: 24px;
    border-radius: var(--radius-sm);
    color: var(--text-tertiary);
  }

  .icon-button:hover {
    color: var(--text-primary);
    background: var(--bg-elevated);
  }

  .shell {
    grid-template-columns: 64px 280px minmax(0, 1fr) 320px;
    background: var(--bg-app);
    transition: grid-template-columns 120ms ease;
  }

  .shell.left-collapsed {
    grid-template-columns: 64px 0 minmax(0, 1fr) 320px;
  }

  .shell.right-collapsed {
    grid-template-columns: 64px 280px minmax(0, 1fr) 0;
  }

  .shell.left-collapsed.right-collapsed {
    grid-template-columns: 64px 0 minmax(0, 1fr) 0;
  }

  .shell.wide-workspace,
  .shell.wide-workspace.left-collapsed,
  .shell.wide-workspace.right-collapsed,
  .shell.wide-workspace.left-collapsed.right-collapsed {
    grid-template-columns: 64px 0 minmax(0, 1fr) 0;
  }

  .shell.wide-workspace .context-panel,
  .shell.wide-workspace .inspector {
    border: 0;
    pointer-events: none;
  }

  .nav-rail,
  .context-panel,
  .inspector {
    border-color: var(--border-default);
    background: var(--bg-panel);
  }

  .nav-rail {
    width: 64px;
    padding: 8px 0;
  }

  .rail-stack {
    gap: 3px;
  }

  .rail-button {
    position: relative;
    width: 64px;
    height: 40px;
    border-width: 0 0 0 2px;
    border-radius: 0;
    border-left-color: transparent;
    color: var(--text-tertiary);
  }

  .rail-button::before {
    content: "";
    position: absolute;
    inset: 8px auto 8px 0;
    width: 2px;
    border-radius: 0;
    background: transparent;
  }

  .rail-button:hover {
    color: var(--text-primary);
    background: var(--bg-elevated);
  }

  .rail-button.active {
    color: var(--accent-primary);
    border-left-color: var(--accent-primary);
    background: var(--bg-elevated);
  }

  .rail-button.active::before {
    background: var(--accent-primary);
  }

  .rail-button.monitor {
    color: var(--accent-info);
  }

  .panel-header {
    height: 36px;
    padding: 0 10px;
    border-color: var(--border-default);
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .context-panel > :not(.panel-header),
  .inspector-section {
    margin: 10px;
  }

  .workspace {
    min-width: 0;
    padding: 0;
    background: var(--bg-panel);
  }

  .workspace-header {
    min-height: 58px;
    padding: 12px 14px;
    border-bottom: 1px solid var(--border-default);
    background: var(--bg-panel);
  }

  .workspace-header h1 {
    margin: 0;
    color: var(--text-primary);
    font-size: var(--text-xl);
    font-weight: 650;
    letter-spacing: 0;
  }

  .eyebrow,
  .section-label {
    color: var(--text-tertiary);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  select,
  input,
  textarea {
    height: 32px;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    background: var(--bg-elevated);
    font-family: var(--font-mono);
    font-size: var(--text-sm);
  }

  textarea {
    height: auto;
    min-height: 64px;
    line-height: 1.55;
  }

  .tool-button,
  .primary-button,
  .send-button,
  .power-button,
  .segmented button,
  .inspector-action {
    min-height: 32px;
    border-radius: var(--radius-sm);
    font-size: var(--text-sm);
  }

  .tool-button,
  .segmented button,
  .inspector-action.secondary {
    border: 1px solid var(--border-default);
    color: var(--text-primary);
    background: var(--bg-elevated);
  }

  .tool-button:hover,
  .segmented button:hover,
  .inspector-action.secondary:hover {
    border-color: var(--border-strong);
    background: var(--bg-active);
  }

  .primary-button,
  .send-button,
  .inspector-action {
    border: 0;
    color: #020617;
    background: var(--accent-primary);
    font-weight: 700;
  }

  .power-button {
    border: 1px solid var(--border-default);
    color: var(--text-primary);
    background: var(--bg-elevated);
  }

  .power-button.online,
  .primary-button.online {
    color: #020617;
    background: var(--accent-primary);
  }

  .search-box,
  .model-import-box > div,
  .kb-create-row,
  .api-url,
  .retrieval-query {
    border-color: var(--border-default);
    border-radius: var(--radius-sm);
    background: var(--bg-elevated);
  }

  .history-item,
  .model-mini,
  .endpoint-row,
  .document-list button,
  .retrieval-results article,
  .rag-metric-grid div,
  .load-plan,
  .hardware-plan,
  .profile-strip,
  .distribution-panel,
  .schema-editor > div,
  .api-dashboard,
  .benchmark-bars div {
    border-color: var(--border-default);
    border-radius: var(--radius-md);
    background: var(--bg-panel);
    box-shadow: none;
  }

  .history-item:hover,
  .model-mini:hover,
  .model-row:hover {
    color: var(--text-primary);
    background: var(--bg-elevated);
  }

  .history-item.active,
  .model-mini.active,
  .model-row.active {
    border-color: var(--border-strong);
    background: var(--bg-active);
  }

  .model-table {
    display: grid;
    gap: 0;
    padding: 0;
    border-top: 1px solid var(--border-default);
    border-bottom: 1px solid var(--border-default);
  }

  .table-row {
    min-height: 38px;
    display: grid;
    grid-template-columns: 48px minmax(240px, 1.7fr) 96px 100px 86px 110px 86px;
    gap: 8px;
    align-items: center;
    padding: 0 12px;
    border: 0;
    border-bottom: 1px solid var(--border-default);
    border-radius: 0;
    background: transparent;
  }

  .table-row.header {
    min-height: 32px;
    color: var(--text-tertiary);
    background: var(--bg-app);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }

  .model-row {
    color: var(--text-secondary);
    text-align: left;
    cursor: pointer;
  }

  .model-status-dot {
    width: 9px;
    height: 9px;
    display: inline-block;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-pill);
    background: var(--text-tertiary);
  }

  .model-status-dot.online-dot {
    border-color: color-mix(in srgb, var(--accent-primary) 64%, var(--border-default));
    background: var(--accent-primary);
  }

  .quant-badge {
    width: fit-content;
    max-width: 100%;
    padding: 2px 6px;
    border: 1px solid color-mix(in srgb, var(--accent-info) 32%, var(--border-default));
    border-radius: var(--radius-pill);
    color: var(--accent-info);
    background: color-mix(in srgb, var(--accent-info) 8%, transparent);
  }

  .row-action {
    height: 28px;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    color: #020617;
    background: var(--accent-primary);
    font-size: var(--text-xs);
    font-weight: 750;
    cursor: pointer;
  }

  .row-action.danger {
    color: #020617;
    background: var(--accent-warning);
  }

  .model-name-cell strong,
  .model-name-cell small,
  .endpoint-row span,
  .endpoint-row small,
  .stat-row span,
  .stat-row code,
  .statusbar span,
  .statusbar code {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  code,
  pre,
  .log-console,
  .log-line,
  .statusbar {
    font-family: var(--font-mono);
  }

  pre,
  .log-console {
    border-color: var(--border-default);
    background: var(--bg-app);
  }

  .log-console {
    padding: 10px;
    font-size: 12px;
    line-height: 1.4;
  }

  .log-line {
    min-height: 24px;
    grid-template-columns: 116px 58px 110px minmax(0, 1fr);
    border-bottom: 1px solid color-mix(in srgb, var(--border-default) 70%, transparent);
  }

  .log-line strong {
    color: var(--accent-info);
  }

  .log-line.warn strong {
    color: var(--accent-warning);
  }

  .log-line.error strong {
    color: var(--accent-danger);
  }

  .chat-panel,
  .prompt-panel,
  .context-strip,
  .metric-stack,
  .inspector-section {
    border-color: var(--border-default);
    border-radius: var(--radius-md);
    background: var(--bg-panel);
  }

  .message.user {
    width: min(760px, 82%);
    margin-left: auto;
    padding: 10px 12px;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-md);
    background: var(--bg-elevated);
  }

  .message.assistant {
    width: min(860px, 94%);
    border-bottom: 1px solid var(--border-default);
    background: transparent;
  }

  .message.system {
    max-width: 100%;
    padding: 8px 10px;
    border: 1px dashed var(--border-default);
    border-radius: var(--radius-sm);
    color: var(--text-tertiary);
    background: color-mix(in srgb, var(--bg-elevated) 50%, transparent);
  }

  .message.streaming::after {
    content: "";
    display: inline-block;
    width: 2px;
    height: 1em;
    margin-left: 3px;
    vertical-align: -0.15em;
    background: var(--accent-info);
    animation: cockpit-caret 900ms steps(2, start) infinite;
  }

  .prompt-dock {
    padding: 0 14px 14px;
  }

  .prompt-row {
    grid-template-columns: minmax(0, 1fr) 240px 44px;
    align-items: stretch;
  }

  .prompt-row textarea {
    min-height: 72px;
    max-height: 200px;
    padding: 10px 12px;
    border-color: var(--border-strong);
    resize: vertical;
  }

  .prompt-tools {
    display: grid;
    grid-template-rows: 32px minmax(0, 1fr);
    gap: 8px;
  }

  .prompt-tools label {
    min-height: 32px;
    display: grid;
    grid-template-columns: 42px minmax(0, 1fr) 42px;
    align-items: center;
    gap: 8px;
    padding: 0 8px;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    background: var(--bg-elevated);
    text-transform: none;
  }

  .prompt-tools input[type="range"] {
    height: 18px;
    padding: 0;
  }

  .prompt-tools code {
    color: var(--accent-info);
    font-size: var(--text-xs);
    text-align: right;
  }

  .message .stream-cursor {
    display: none;
  }

  @keyframes cockpit-caret {
    50% {
      opacity: 0;
    }
  }

  .segment-bar,
  .memory-bar,
  .mini-bar,
  .status-meter {
    overflow: hidden;
    border: 1px solid var(--border-default);
    background: var(--bg-app);
  }

  .statusbar {
    height: 24px;
    border-top: 1px solid var(--border-default);
    background: var(--bg-app);
    color: var(--text-tertiary);
    font-size: var(--text-xs);
  }

  .statusbar .online {
    color: var(--accent-primary);
  }

  .command-palette {
    border-color: var(--border-strong);
    border-radius: var(--radius-md);
    background: var(--bg-panel);
  }

  .palette-input,
  .palette-results button {
    border-color: var(--border-default);
  }

  .palette-results button:hover {
    color: var(--text-primary);
    background: var(--bg-elevated);
  }

  .api-dashboard {
    width: min(800px, calc(100% - 28px));
    justify-self: center;
    margin: 14px auto;
  }

  .api-dashboard::before {
    content: "";
    width: 10px;
    height: 10px;
    display: inline-block;
    margin-right: 8px;
    border-radius: var(--radius-pill);
    border: 1px solid color-mix(in srgb, var(--accent-danger) 42%, var(--border-default));
    background: var(--accent-danger);
  }

  .api-dashboard.online::before {
    border-color: color-mix(in srgb, var(--accent-primary) 46%, var(--border-default));
    background: var(--accent-primary);
  }

  .curl-block {
    min-height: 128px;
    max-width: 100%;
    overflow: auto;
    color: var(--text-secondary);
    white-space: pre;
  }

  .benchmark-metrics {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 10px;
    padding: 14px;
  }

  .benchmark-metrics article {
    display: grid;
    gap: 6px;
    padding: 12px;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-md);
    background: var(--bg-panel);
  }

  .benchmark-metrics span {
    color: var(--text-tertiary);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: 0.07em;
    text-transform: uppercase;
  }

  .benchmark-metrics strong {
    color: var(--text-primary);
    font-family: var(--font-mono);
    font-size: 18px;
  }

  .settings-workspace {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
    padding: 14px;
  }

  .settings-workspace article {
    display: grid;
    align-content: start;
    gap: 12px;
    min-height: 210px;
    padding: 14px;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-md);
    background: var(--bg-panel);
  }

  .settings-workspace label {
    display: grid;
    gap: 6px;
  }

  .settings-workspace .toggle-line {
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
  }

  .path-row {
    display: grid;
    grid-template-columns: 110px minmax(0, 1fr);
    gap: 10px;
    align-items: center;
    min-height: 32px;
    padding: 0 8px;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    background: var(--bg-elevated);
  }

  .path-row span {
    color: var(--text-tertiary);
    font-size: var(--text-xs);
  }

  .path-row code {
    overflow: hidden;
    color: var(--text-secondary);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .schema-editor pre {
    max-height: 360px;
    font-size: 11px;
  }

  .rag-workbench,
  .knowledge-workbench {
    padding: 14px;
  }

  .rag-workbench > * {
    min-height: 260px;
    border-radius: var(--radius-md);
  }

  .agent-canvas {
    background-image: radial-gradient(color-mix(in srgb, var(--text-tertiary) 28%, transparent) 1px, transparent 1px);
    background-size: 18px 18px;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-md);
    margin: 14px;
    padding: 14px;
  }

  @media (max-width: 1439px) {
    .shell {
      grid-template-columns: 64px 280px minmax(0, 1fr) 0;
    }

    .inspector {
      border-left: 0;
      pointer-events: none;
    }

    .blueprint-grid,
    .rag-workbench,
    .api-config-grid,
    .load-plan-grid,
    .allocation-grid,
    .runtime-grid {
      grid-template-columns: 1fr;
    }

    .benchmark-metrics,
    .settings-workspace {
      grid-template-columns: 1fr;
      width: 100%;
    }

    .tuning-grid,
    .schema-editor {
      grid-template-columns: 1fr;
    }

    .prompt-row {
      grid-template-columns: minmax(0, 1fr) 44px;
    }

    .prompt-tools {
      grid-column: 1 / -1;
      grid-row: 2;
      grid-template-columns: minmax(0, 1fr) minmax(180px, 0.6fr);
      grid-template-rows: auto;
    }
  }

  @media (max-width: 1279px) {
    .titlebar {
      grid-template-columns: minmax(220px, 0.9fr) minmax(260px, 1fr) minmax(160px, auto);
    }

    .title-model code,
    .title-context-meter {
      display: none;
    }

    .shell,
    .shell.right-collapsed {
      grid-template-columns: 64px 0 minmax(0, 1fr) 0;
    }

    .context-panel,
    .inspector {
      border: 0;
      pointer-events: none;
    }

    .table-row,
    .endpoint-row,
    .log-line {
      grid-template-columns: minmax(0, 1fr);
      gap: 4px;
      padding: 8px 10px;
    }

    .prompt-tools {
      grid-template-columns: 1fr;
    }
  }

  /* Instrument workbench refinement layer */
  .telemetry-board {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 360px), 1fr));
    gap: 0;
    margin: 14px;
    border-top: 1px solid var(--border-default);
    border-left: 1px solid var(--border-default);
    background: var(--bg-app);
  }

  .scope-panel {
    min-height: 252px;
    display: grid;
    grid-template-rows: 32px minmax(118px, 1fr) 18px auto auto auto;
    gap: 8px;
    padding: 10px;
    border-right: 1px solid var(--border-default);
    border-bottom: 1px solid var(--border-default);
    background: var(--bg-panel);
  }

  .scope-head,
  .scope-axis,
  .scope-meta {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    min-width: 0;
  }

  .scope-head span,
  .scope-axis,
  .scope-meta span,
  .scope-panel code {
    color: var(--text-tertiary);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
  }

  .scope-head strong,
  .scope-meta strong {
    overflow: hidden;
    color: var(--text-primary);
    font-family: var(--font-mono);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .scope-line {
    width: 100%;
    height: 100%;
    min-height: 118px;
    border: 1px solid var(--border-default);
    background:
      linear-gradient(var(--border-default) 1px, transparent 1px),
      linear-gradient(90deg, var(--border-default) 1px, transparent 1px),
      var(--bg-app);
    background-size: 24px 24px;
  }

  .scope-line polyline {
    fill: none;
    stroke: var(--accent-primary);
    stroke-linecap: square;
    stroke-linejoin: miter;
    stroke-width: 1;
    vector-effect: non-scaling-stroke;
  }

  .precise-slider {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 92px;
    align-items: center;
    gap: 8px;
  }

  .memory-bar,
  .segment-bar {
    height: 10px;
    border-radius: var(--radius-sm);
  }

  .memory-bar span,
  .segment-bar span {
    background: var(--accent-primary);
  }

  .control-band,
  .hardware-plan {
    margin-right: 14px;
    margin-left: 14px;
    border-radius: var(--radius-sm);
  }

  .agent-workbench {
    min-height: 0;
    display: grid;
    grid-template-columns: 240px minmax(0, 1fr) 320px;
    gap: 0;
    margin: 14px;
    border: 1px solid var(--border-default);
    background: var(--bg-panel);
  }

  .agent-list,
  .agent-detail,
  .tool-schema-list {
    min-width: 0;
    display: grid;
    align-content: start;
    gap: 10px;
    padding: 12px;
  }

  .agent-detail,
  .tool-schema-list {
    border-left: 1px solid var(--border-default);
  }

  .agent-row,
  .tool-schema-row {
    width: 100%;
    min-height: 40px;
    display: grid;
    align-items: center;
    gap: 8px;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    background: var(--bg-elevated);
  }

  .agent-row {
    grid-template-columns: 20px minmax(0, 1fr) auto;
    padding: 8px;
    text-align: left;
    cursor: pointer;
  }

  .agent-row.active {
    border-color: var(--border-strong);
    color: var(--text-primary);
    background: var(--bg-active);
  }

  .agent-row span,
  .tool-schema-row span {
    min-width: 0;
    display: grid;
    gap: 2px;
  }

  .agent-row strong,
  .agent-row small,
  .tool-schema-row strong,
  .tool-schema-row small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .agent-row small,
  .tool-schema-row small {
    color: var(--text-tertiary);
    font-size: var(--text-xs);
  }

  .agent-detail label {
    display: grid;
    gap: 6px;
    text-transform: none;
  }

  .agent-detail textarea {
    min-height: 128px;
    resize: vertical;
  }

  .tool-schema-row {
    grid-template-columns: minmax(0, 1fr) 22px;
    padding: 8px;
  }

  .tool-schema-row.danger {
    border-color: color-mix(in srgb, var(--accent-danger) 42%, var(--border-default));
  }

  .tool-schema-list pre,
  .agent-detail pre {
    min-height: auto;
    max-height: 180px;
    border-radius: var(--radius-sm);
    font-size: var(--text-xs);
  }

  .api-dashboard {
    width: auto;
    margin: 14px;
    padding: 0;
    border-radius: var(--radius-sm);
    background: var(--bg-panel);
  }

  .api-dashboard::before {
    display: none;
  }

  .api-status-strip {
    min-height: 38px;
    display: grid;
    grid-template-columns: 108px minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    padding: 0 12px;
    border-bottom: 1px solid var(--border-default);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
  }

  .api-status-strip span {
    color: var(--accent-danger);
    text-transform: uppercase;
  }

  .api-dashboard.online .api-status-strip span {
    color: var(--accent-primary);
  }

  .api-status-strip code {
    overflow: hidden;
    color: var(--text-secondary);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .api-status-strip button {
    height: 28px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 0 9px;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    background: var(--bg-elevated);
    cursor: pointer;
  }

  .api-config-grid {
    grid-template-columns: minmax(0, 1.2fr) minmax(120px, 0.45fr) auto;
    padding: 12px;
    border-bottom: 1px solid var(--border-default);
  }

  .api-split {
    display: grid;
    grid-template-columns: minmax(320px, 0.92fr) minmax(0, 1.08fr);
    min-height: 360px;
  }

  .api-split .endpoint-table,
  .api-example {
    min-width: 0;
    padding: 12px;
  }

  .api-example {
    border-left: 1px solid var(--border-default);
  }

  .endpoint-row {
    min-height: 36px;
    border-radius: var(--radius-sm);
    background: var(--bg-elevated);
  }

  .curl-block {
    height: calc(100% - 28px);
    min-height: 260px;
    border-radius: var(--radius-sm);
    white-space: pre;
  }

  .benchmark-metrics {
    gap: 0;
    margin: 14px 14px 0;
    padding: 0;
    border-top: 1px solid var(--border-default);
    border-left: 1px solid var(--border-default);
  }

  .benchmark-metrics article {
    min-height: 72px;
    border-width: 0 1px 1px 0;
    border-radius: 0;
    background: var(--bg-panel);
  }

  .benchmark-bars {
    display: grid;
    gap: 0;
    margin: 0 14px 14px;
    border-top: 1px solid var(--border-default);
    border-left: 1px solid var(--border-default);
    background: var(--bg-panel);
  }

  .benchmark-bars .benchmark-row {
    min-height: 38px;
    display: grid;
    grid-template-columns: minmax(220px, 1.2fr) minmax(160px, 1fr) 110px 110px 76px;
    gap: 10px;
    align-items: center;
    padding: 0 12px;
    border-right: 1px solid var(--border-default);
    border-bottom: 1px solid var(--border-default);
  }

  .benchmark-head {
    color: var(--text-tertiary);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    background: var(--bg-app);
  }

  .benchmark-bars .benchmark-row > * {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .benchmark-bars i {
    height: 12px;
    display: block;
    border-radius: var(--radius-sm);
    background: var(--accent-primary);
  }

  .settings-workspace {
    gap: 0;
    padding: 14px 14px 64px;
    align-content: start;
  }

  .settings-workspace article {
    min-height: 190px;
    border-radius: var(--radius-sm);
  }

  .settings-savebar {
    position: sticky;
    bottom: 0;
    z-index: 2;
    grid-column: 1 / -1;
    min-height: 44px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 6px 10px;
    border: 1px solid var(--border-default);
    border-top: 0;
    color: var(--text-secondary);
    background: var(--bg-app);
    font-size: var(--text-xs);
  }

  .shell.wide-workspace .workspace {
    overflow: hidden;
  }

  .log-summary {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-tertiary);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
  }

  .log-summary code {
    padding: 2px 6px;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    background: var(--bg-elevated);
  }

  .log-workbench {
    min-height: 0;
    flex: 1 1 auto;
    display: grid;
    grid-template-columns: 112px minmax(0, 1fr);
    margin: 0;
    border-top: 1px solid var(--border-default);
    background: var(--bg-app);
  }

  .log-level-rail {
    min-height: 0;
    display: grid;
    align-content: start;
    gap: 4px;
    padding: 10px;
    border-right: 1px solid var(--border-default);
    background: var(--bg-panel);
  }

  .log-level-rail button {
    min-height: 34px;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    padding: 0 8px;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    color: var(--text-tertiary);
    background: transparent;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    text-align: left;
    cursor: pointer;
  }

  .log-level-rail button:hover,
  .log-level-rail button.active {
    border-color: var(--border-strong);
    color: var(--text-primary);
    background: var(--bg-elevated);
  }

  .log-level-rail button.active {
    border-left-color: var(--accent-primary);
    color: var(--accent-primary);
  }

  .log-level-rail code {
    color: var(--text-tertiary);
    font-size: 10px;
  }

  .log-terminal {
    min-width: 0;
    min-height: 0;
    display: grid;
    grid-template-rows: 28px minmax(0, 1fr) 40px;
    background: var(--bg-app);
  }

  .log-terminal-head,
  .log-line {
    display: grid;
    grid-template-columns: 162px 64px 150px minmax(0, 1fr);
    gap: 10px;
    align-items: start;
  }

  .log-terminal-head {
    align-items: center;
    padding: 0 12px;
    border-bottom: 1px solid var(--border-default);
    color: var(--text-tertiary);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    background: var(--bg-panel);
  }

  .log-console {
    min-height: 0;
    overflow: auto;
    padding: 0;
    border: 0;
    background: var(--bg-app);
    font-size: 13px;
    line-height: 1.45;
  }

  .log-line {
    min-height: 28px;
    padding: 5px 12px;
    border-bottom: 1px solid color-mix(in srgb, var(--border-default) 72%, transparent);
    color: var(--text-secondary);
    font-family: var(--font-mono);
  }

  .log-line:hover {
    background: color-mix(in srgb, var(--bg-elevated) 58%, transparent);
  }

  .log-line code,
  .log-line span,
  .log-line strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .log-line strong {
    color: var(--text-secondary);
    font-size: var(--text-xs);
    font-weight: 700;
  }

  .log-line.warn strong {
    color: var(--accent-warning);
  }

  .log-line.error strong {
    color: var(--accent-danger);
  }

  .log-line.debug strong {
    color: var(--text-tertiary);
  }

  .log-line p {
    overflow-wrap: anywhere;
  }

  .log-empty {
    min-height: 100%;
    display: grid;
    place-items: center;
    align-content: center;
    gap: 10px;
    color: var(--text-tertiary);
    font-family: var(--font-mono);
    font-size: var(--text-sm);
  }

  .log-command-bar {
    display: grid;
    grid-template-columns: 56px minmax(0, 1fr) 76px;
    align-items: center;
    gap: 8px;
    padding: 4px 8px;
    border-top: 1px solid var(--border-default);
    background: var(--bg-panel);
  }

  .log-command-bar > code {
    color: var(--accent-primary);
    font-size: var(--text-xs);
    text-align: center;
  }

  .log-command-bar input {
    height: 30px;
  }

  .document-tree-list {
    display: grid;
    gap: 4px;
  }

  .document-tree-list button {
    min-height: 30px;
    display: grid;
    grid-template-columns: 16px minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    padding: 0 8px;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    background: transparent;
    text-align: left;
    cursor: pointer;
  }

  .document-tree-list button:hover,
  .document-tree-list button.active {
    color: var(--text-primary);
    background: var(--bg-elevated);
  }

  .document-tree-list span,
  .document-tree-list code {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .rag-workbench {
    min-height: 0;
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr) 260px;
    gap: 0;
    margin: 14px;
    padding: 0;
    border: 1px solid var(--border-default);
    background: var(--bg-panel);
  }

  .rag-doc-tree,
  .chunk-browser,
  .retrieval-panel {
    min-width: 0;
    min-height: 0;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    align-content: start;
    gap: 10px;
    padding: 12px;
    border-radius: 0;
    background: var(--bg-panel);
  }

  .chunk-browser,
  .retrieval-panel {
    border-left: 1px solid var(--border-default);
  }

  .document-list,
  .chunk-list,
  .retrieval-results {
    min-height: 0;
    display: grid;
    align-content: start;
    gap: 6px;
    overflow: auto;
  }

  .document-list button {
    min-height: 58px;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 3px 8px;
    padding: 8px;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    background: var(--bg-elevated);
    text-align: left;
    cursor: pointer;
  }

  .document-list button:hover,
  .document-list button.active {
    border-color: var(--border-strong);
    color: var(--text-primary);
    background: var(--bg-active);
  }

  .document-list strong,
  .document-list span,
  .document-list code {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .document-list strong {
    grid-column: 1 / -1;
    color: var(--text-primary);
    font-size: var(--text-sm);
  }

  .document-list span,
  .document-list code {
    color: var(--text-tertiary);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
  }

  .chunk-row {
    display: grid;
    gap: 7px;
    padding: 8px;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    background: var(--bg-app);
  }

  .chunk-row.active {
    border-color: color-mix(in srgb, var(--accent-primary) 42%, var(--border-default));
    background: color-mix(in srgb, var(--accent-primary) 8%, var(--bg-app));
  }

  .chunk-row > div,
  .retrieval-results .panel-header.inline {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin: 0;
  }

  .chunk-row span,
  .chunk-row code {
    color: var(--text-tertiary);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
  }

  .chunk-row p,
  .retrieval-results p {
    display: -webkit-box;
    overflow: hidden;
    margin: 0;
    color: var(--text-secondary);
    font-size: var(--text-xs);
    line-height: 1.5;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 3;
    line-clamp: 3;
  }

  .chunk-row i,
  .retrieval-results article > i {
    overflow: hidden;
    height: 4px;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    background: var(--bg-elevated);
  }

  .chunk-row i b,
  .retrieval-results article > i b {
    display: block;
    height: 100%;
    background: var(--accent-primary);
  }

  .retrieval-panel {
    grid-template-rows: auto auto auto minmax(0, 1fr);
  }

  .retrieval-query {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 64px;
    gap: 8px;
    padding: 0;
    border: 0;
    background: transparent;
  }

  .rag-metric-grid {
    grid-template-columns: 1fr;
    gap: 6px;
  }

  .rag-metric-grid div {
    min-height: 42px;
    display: grid;
    gap: 2px;
    padding: 7px 8px;
    border-radius: var(--radius-sm);
    background: var(--bg-elevated);
  }

  .retrieval-results article {
    display: grid;
    gap: 7px;
    padding: 8px;
    border-radius: var(--radius-sm);
    background: var(--bg-elevated);
  }

  .retrieval-results .panel-header.inline span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  @media (max-width: 1439px) {
    .telemetry-board,
    .agent-workbench,
    .api-split,
    .rag-workbench {
      grid-template-columns: 1fr;
    }

    .agent-detail,
    .tool-schema-list,
    .api-example,
    .chunk-browser,
    .retrieval-panel {
      border-left: 0;
      border-top: 1px solid var(--border-default);
    }

    .benchmark-metrics {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }

  @media (max-width: 1279px) {
    .telemetry-board,
    .agent-workbench,
    .rag-workbench,
    .api-dashboard,
    .benchmark-metrics,
    .benchmark-bars {
      margin-right: 10px;
      margin-left: 10px;
    }

    .benchmark-bars .benchmark-row {
      grid-template-columns: minmax(0, 1fr);
      gap: 4px;
      padding: 8px 10px;
    }

    .benchmark-head {
      display: none;
    }

    .api-status-strip,
    .precise-slider,
    .api-config-grid {
      grid-template-columns: 1fr;
      align-items: start;
      padding-top: 8px;
      padding-bottom: 8px;
    }

    .settings-savebar {
      align-items: stretch;
      flex-direction: column;
    }

    .log-workbench {
      grid-template-columns: 88px minmax(0, 1fr);
    }

    .log-level-rail {
      padding: 8px 6px;
    }

    .log-level-rail button {
      grid-template-columns: 1fr;
      justify-items: start;
      gap: 2px;
      padding: 5px 6px;
    }

    .log-terminal-head,
    .log-line {
      grid-template-columns: 132px 54px minmax(0, 1fr);
    }

    .log-terminal-head span:nth-child(3),
    .log-line > span {
      display: none;
    }

    .log-command-bar {
      grid-template-columns: 46px minmax(0, 1fr) 64px;
    }
  }
</style>
