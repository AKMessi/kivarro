<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import {
    Activity,
    Archive,
    Boxes,
    BrainCircuit,
    ChevronDown,
    Circle,
    Clipboard,
    Command,
    Cpu,
    Database,
    FileText,
    FolderOpen,
    Gauge,
    HardDrive,
    Layers3,
    Maximize2,
    Minus,
    Moon,
    Network,
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
    Wrench,
    X,
    Zap,
  } from "@lucide/svelte";
  import {
    getApiStatus,
    getHardwareSnapshot,
    getRuntimeMetrics,
    listBenchmarkResults,
    listModels,
    listSystemLogs,
  } from "$lib/api";
  import type {
    ApiStatus,
    BenchmarkResult,
    HardwareSnapshot,
    LogEntry,
    ModelRecord,
    RuntimeMetrics,
    ViewId,
  } from "$lib/types";

  type NavItem = {
    id: ViewId;
    label: string;
    icon: typeof Activity;
  };

  type ChatMessage = {
    id: string;
    role: "user" | "assistant" | "system";
    label: string;
    content: string;
    tokens?: number;
    speed?: number;
  };

  const navItems: NavItem[] = [
    { id: "command", label: "Command Center", icon: Command },
    { id: "models", label: "Model Registry", icon: Archive },
    { id: "hardware", label: "Hardware Fit", icon: Cpu },
    { id: "tuning", label: "Expert Tuning", icon: SlidersHorizontal },
    { id: "knowledge", label: "Knowledge Base", icon: Database },
    { id: "agents", label: "Agents", icon: BrainCircuit },
    { id: "api", label: "Local API", icon: Server },
    { id: "benchmarks", label: "Benchmarks", icon: Gauge },
    { id: "logs", label: "System Logs", icon: Terminal },
    { id: "settings", label: "Settings", icon: Settings },
  ];

  const chatHistory = [
    { title: "Today", items: ["Inference scratchpad", "Schema extraction test", "Qwen coding profile"] },
    { title: "Previous 7 Days", items: ["Long context summary", "RAG retrieval audit"] },
  ];

  const promptProfiles = [
    "Balanced Engineer",
    "Strict JSON Extractor",
    "Local Code Reviewer",
    "Long Context Analyst",
  ];

  const knowledgeBases = ["Research Vault", "Codebase Memory", "Paper Notes"];
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
  let splitView = false;
  let promptText = "";
  let selectedProfile = promptProfiles[0];
  let modelFilter = "";
  let logFilter = "ALL";

  let hardware: HardwareSnapshot | null = null;
  let metrics: RuntimeMetrics | null = null;
  let models: ModelRecord[] = [];
  let apiStatus: ApiStatus | null = null;
  let benchmarks: BenchmarkResult[] = [];
  let logs: LogEntry[] = [];

  let sampling = {
    temperature: 0.7,
    topP: 0.92,
    topK: 40,
    minP: 0.05,
    repeatPenalty: 1.1,
    presencePenalty: 0,
    frequencyPenalty: 0,
    maxTokens: 2048,
    gpuLayers: 0,
    contextLength: 32768,
  };

  const chatMessages: ChatMessage[] = [
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
  $: contextPercent = metrics
    ? clamp((metrics.contextUsedTokens / metrics.contextTotalTokens) * 100, 0, 100)
    : 0;
  $: ramPercent = metrics ? clamp((metrics.ramUsedGib / Math.max(metrics.ramTotalGib, 1)) * 100, 0, 100) : 0;
  $: filteredModels = models.filter((model) =>
    `${model.name} ${model.format} ${model.path}`.toLowerCase().includes(modelFilter.toLowerCase()),
  );
  $: filteredLogs =
    logFilter === "ALL" ? logs : logs.filter((entry) => entry.level.toUpperCase() === logFilter);
  $: document.documentElement.dataset.theme = theme;

  onMount(() => {
    void hydrate();
    const refreshTimer = window.setInterval(() => void refreshRuntime(), 4000);
    const keyHandler = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        commandPaletteOpen = !commandPaletteOpen;
      }
    };

    window.addEventListener("keydown", keyHandler);

    return () => {
      window.clearInterval(refreshTimer);
      window.removeEventListener("keydown", keyHandler);
    };
  });

  async function hydrate() {
    const [nextHardware, nextMetrics, nextModels, nextApiStatus, nextBenchmarks, nextLogs] =
      await Promise.all([
        getHardwareSnapshot(),
        getRuntimeMetrics(),
        listModels(),
        getApiStatus(),
        listBenchmarkResults(),
        listSystemLogs(),
      ]);

    hardware = nextHardware;
    metrics = nextMetrics;
    models = nextModels;
    apiStatus = nextApiStatus;
    benchmarks = nextBenchmarks;
    logs = nextLogs;
  }

  async function refreshRuntime() {
    metrics = await getRuntimeMetrics();
  }

  function setActiveView(view: ViewId) {
    activeView = view;
    commandPaletteOpen = false;
  }

  function toggleTheme() {
    theme = theme === "dark" ? "light" : "dark";
  }

  function submitPrompt() {
    if (!promptText.trim()) return;
    promptText = "";
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
    if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) {
      return null;
    }

    return getCurrentWindow();
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
    <div class="window-controls">
      <button aria-label="Close window" class="window-control close" onclick={closeWindow}>
        <X size={12} />
      </button>
      <button aria-label="Minimize window" class="window-control minimize" onclick={minimizeWindow}>
        <Minus size={12} />
      </button>
      <button aria-label="Maximize window" class="window-control maximize" onclick={toggleMaximizeWindow}>
        <Maximize2 size={11} />
      </button>
    </div>

    <div class="title-identity" data-tauri-drag-region>
      <span class="wordmark">Kivarro</span>
      <span class="title-divider"></span>
      <span class="active-view">{activeMeta.label}</span>
    </div>

    <div class="quick-actions">
      <button class="icon-button" aria-label="Toggle left panel" title="Toggle left panel" onclick={() => (leftCollapsed = !leftCollapsed)}>
        <PanelLeftClose size={16} />
      </button>
      <button class="icon-button" aria-label="Command palette" title="Cmd/Ctrl + K" onclick={() => (commandPaletteOpen = !commandPaletteOpen)}>
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
  </header>

  <div class="shell" class:left-collapsed={leftCollapsed} class:right-collapsed={rightCollapsed}>
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
            <svelte:component this={item.icon} size={18} strokeWidth={1.8} />
          </button>
        {/each}
      </div>
      <button class="rail-button monitor" aria-label="Hardware status monitor" title="Hardware status">
        <Activity size={18} strokeWidth={1.8} />
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
          <select id="profile" bind:value={selectedProfile}>
            {#each promptProfiles as profile}
              <option value={profile}>{profile}</option>
            {/each}
          </select>
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
        <button class="drop-zone">
          <Upload size={18} />
          <span>Drop .gguf or browse</span>
        </button>
        <div class="section-label">Discovered</div>
        {#if models.length === 0}
          <p class="muted-copy">No models found in ./models yet.</p>
        {:else}
          {#each filteredModels as model}
            <button class="model-mini">
              <span>{model.name}</span>
              <small>{model.format} / {formatNumber(model.sizeGib)} GiB</small>
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
        {#each knowledgeBases as base}
          <button class="history-item">
            <Database size={14} />
            {base}
          </button>
        {/each}
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
            <button class="primary-button">
              <Play size={15} />
              Load model
            </button>
          </div>
        </section>

        <section class:split={splitView} class="chat-surface">
          <div class="chat-pane">
            <div class="pane-header">
              <span>{metrics?.activeModel ?? "No model loaded"}</span>
              <code>{formatNumber(metrics?.tokensPerSecond ?? 0)} tok/s</code>
            </div>
            <div class="message-list">
              {#each chatMessages as message}
                <article class:system={message.role === "system"} class="message">
                  <div class="message-meta">
                    <span>{message.label}</span>
                    {#if message.tokens}
                      <code>{message.tokens} tokens</code>
                    {/if}
                  </div>
                  <p>{message.content}<span class="stream-cursor"></span></p>
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
                if ((event.metaKey || event.ctrlKey) && event.key === "Enter") submitPrompt();
              }}
            ></textarea>
            <button class="send-button" aria-label="Send prompt" onclick={submitPrompt}>
              <Send size={18} />
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
              <span>Name</span>
              <span>Format</span>
              <span>Size</span>
              <span>Fit</span>
              <span>Status</span>
            </div>
            {#each filteredModels as model}
              <div class="table-row">
                <span>{model.name}</span>
                <code>{model.format}</code>
                <code>{formatNumber(model.sizeGib)} GiB</code>
                <span class:good={model.fit === "Fits"} class:warn={model.fit !== "Fits"}>{model.fit}</span>
                <span>{model.status}</span>
              </div>
            {/each}
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

        <section class="blueprint-grid">
          {#each hardware?.blocks ?? [] as block}
            <article class="compute-block">
              <div class="block-top">
                <span>{block.kind}</span>
                <code>{formatNumber(block.utilizationPercent)}%</code>
              </div>
              <h2>{block.name}</h2>
              <p>{block.status}</p>
              {#if block.memoryTotalGib}
                <div class="memory-bar">
                  <span style={`width: ${(block.memoryUsedGib ?? 0) / block.memoryTotalGib * 100}%`}></span>
                </div>
                <code>{formatNumber(block.memoryUsedGib ?? 0)} / {formatNumber(block.memoryTotalGib)} GiB</code>
              {/if}
            </article>
          {/each}
        </section>

        <section class="control-band">
          <div>
            <label for="gpu-layers">GPU offload layers</label>
            <input id="gpu-layers" type="range" min="0" max="96" step="1" bind:value={sampling.gpuLayers} />
            <code>{sampling.gpuLayers} layers / estimated VRAM adapter pending</code>
          </div>
          <div>
            <label for="context-length">Context length</label>
            <input id="context-length" type="range" min="4096" max="262144" step="4096" bind:value={sampling.contextLength} />
            <code>{formatTokens(sampling.contextLength)} tokens</code>
          </div>
        </section>
      {:else if activeView === "tuning"}
        <section class="workspace-header">
          <div>
            <p class="eyebrow">Expert Tuning</p>
            <h1>Reusable inference profile</h1>
          </div>
          <button class="primary-button">
            <Clipboard size={15} />
            Save .kivarro.json
          </button>
        </section>

        <section class="tuning-grid">
          <div class="control-matrix">
            {#each [
              ["Temperature", "temperature", 0, 2, 0.01],
              ["Top P", "topP", 0, 1, 0.01],
              ["Top K", "topK", 0, 200, 1],
              ["Min P", "minP", 0, 1, 0.01],
              ["Repeat Penalty", "repeatPenalty", 0.8, 2, 0.01],
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

          <div class="distribution-panel">
            <div class="panel-header inline">
              <span>Token probability distribution</span>
              <code>live preview</code>
            </div>
            <div class="distribution-chart">
              {#each [88, 64, 42, 31, 24, 18, 13, 9, 7, 4] as value, index}
                <span style={`height: ${value}%`} title={`rank ${index + 1}`}></span>
              {/each}
            </div>
          </div>
        </section>

        <section class="schema-editor">
          <div>
            <div class="panel-header inline"><span>JSON schema / GBNF</span><code>validated</code></div>
            <pre>{
`{
  "type": "object",
  "properties": {
    "answer": { "type": "string" },
    "confidence": { "type": "number" }
  },
  "required": ["answer"]
}`}</pre>
          </div>
          <div>
            <div class="panel-header inline"><span>Output preview</span><code>strict</code></div>
            <pre>{
`{
  "answer": "Pending local model output",
  "confidence": 0.0
}`}</pre>
          </div>
        </section>
      {:else if activeView === "knowledge"}
        <section class="workspace-header">
          <div>
            <p class="eyebrow">RAG Knowledge Bases</p>
            <h1>Local retrieval pipeline</h1>
          </div>
          <button class="primary-button">
            <FileText size={15} />
            Import documents
          </button>
        </section>

        <section class="rag-grid">
          {#each ["Documents", "Chunking Strategy", "Embedding Model"] as column, index}
            <article>
              <div class="panel-header inline">
                <span>{column}</span>
                <code>{index === 0 ? "0 files" : "pending"}</code>
              </div>
              <div class="empty-state compact">
                <FolderOpen size={26} />
                <span>{index === 0 ? "Drop PDFs, Markdown, or source files here." : "Configure after documents are attached."}</span>
              </div>
            </article>
          {/each}
        </section>

        <section class="retrieval-dock">
          <input placeholder="Test retrieval query..." />
          <button class="tool-button">Run semantic search</button>
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

        <section class="agent-canvas">
          <article class="agent-node">
            <BrainCircuit size={22} />
            <strong>Analyst Agent</strong>
            <span>Persona, model, tools, and RAG attachments.</span>
          </article>
          <article class="agent-node">
            <Database size={22} />
            <strong>Knowledge</strong>
            <span>Attach local vector stores with citation requirements.</span>
          </article>
          <article class="agent-node">
            <Wrench size={22} />
            <strong>Tools</strong>
            <span>Gate execution with explicit permissions.</span>
          </article>
        </section>

        <section class="tool-permissions">
          {#each agentTools as tool}
            <label class:danger={tool.danger}>
              <span>{tool.name}</span>
              <input type="checkbox" checked={tool.enabled} />
            </label>
          {/each}
        </section>
      {:else if activeView === "api"}
        <section class="workspace-header">
          <div>
            <p class="eyebrow">Local API Server</p>
            <h1>OpenAI-compatible gateway</h1>
          </div>
          <button class:online={apiStatus?.enabled} class="power-button">
            <Power size={16} />
            {apiStatus?.enabled ? "Server on" : "Server off"}
          </button>
        </section>

        <section class="api-dashboard">
          <div class="api-url">
            <span>Base URL</span>
            <code>{apiStatus?.baseUrl ?? "http://127.0.0.1:8080/v1"}</code>
            <button aria-label="Copy base URL"><Clipboard size={15} /></button>
          </div>
          <div class="endpoint-table">
            {#each apiStatus?.endpoints ?? [] as endpoint}
              <div class="endpoint-row">
                <code>{endpoint.method}</code>
                <span>{endpoint.path}</span>
                <small>{endpoint.description}</small>
                <em>{endpoint.status}</em>
              </div>
            {/each}
          </div>
        </section>
      {:else if activeView === "benchmarks"}
        <section class="workspace-header">
          <div>
            <p class="eyebrow">Benchmarks</p>
            <h1>Throughput and load profile</h1>
          </div>
          <button class="primary-button">
            <Gauge size={15} />
            Run benchmark
          </button>
        </section>

        {#if benchmarks.length === 0}
          <section class="empty-state">
            <Gauge size={44} />
            <strong>No benchmark runs yet</strong>
            <span>Run a tokens/sec benchmark after loading a local model.</span>
          </section>
        {:else}
          <section class="benchmark-bars">
            {#each benchmarks as result}
              <div>
                <span>{result.model}</span>
                <i style={`width: ${Math.min(result.tokensPerSecond, 160)}%`}></i>
                <code>{formatNumber(result.tokensPerSecond)} tok/s</code>
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
        </section>

        <section class="log-console">
          {#each filteredLogs as entry}
            <div class:warn={entry.level === "WARN"} class:error={entry.level === "ERROR"} class="log-line">
              <code>{entry.timestamp}</code>
              <strong>{entry.level}</strong>
              <span>{entry.source}</span>
              <p>{entry.message}</p>
            </div>
          {/each}
        </section>
      {:else}
        <section class="workspace-header">
          <div>
            <p class="eyebrow">Settings</p>
            <h1>Application control plane</h1>
          </div>
        </section>
        <section class="settings-grid">
          {#each ["Model directory", "Telemetry", "Appearance", "Security", "Updates", "Backups"] as setting}
            <article>
              <span>{setting}</span>
              <small>Production setting surface reserved for the next implementation pass.</small>
            </article>
          {/each}
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
    <button class="palette-backdrop" aria-label="Close command palette" onclick={() => (commandPaletteOpen = false)}></button>
    <section class="command-palette" aria-label="Command palette">
      <div class="palette-input">
        <Search size={16} />
        <input placeholder="Navigate, load model, switch theme..." />
      </div>
      <div class="palette-results">
        {#each navItems as item}
          <button onclick={() => setActiveView(item.id)}>
            <svelte:component this={item.icon} size={15} />
            <span>Open {item.label}</span>
          </button>
        {/each}
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

  .title-divider {
    width: 1px;
    height: 14px;
    background: var(--border-strong);
  }

  .active-view {
    color: var(--muted);
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

  .shell {
    min-height: 0;
    display: grid;
    grid-template-columns: 56px 280px minmax(0, 1fr) 320px;
    transition: grid-template-columns 160ms ease;
  }

  .shell.left-collapsed {
    grid-template-columns: 56px 0 minmax(0, 1fr) 320px;
  }

  .shell.right-collapsed {
    grid-template-columns: 56px 280px minmax(0, 1fr) 0;
  }

  .shell.left-collapsed.right-collapsed {
    grid-template-columns: 56px 0 minmax(0, 1fr) 0;
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
  .metric-stack,
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
  .drop-zone {
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

  .history-item:hover,
  .model-mini:hover {
    color: var(--text);
    background: var(--panel-2);
  }

  .drop-zone {
    display: grid;
    place-items: center;
    gap: 8px;
    min-height: 92px;
    margin-top: 12px;
    border-color: var(--border);
    border-style: dashed;
    color: var(--muted);
  }

  .model-mini {
    display: grid;
    gap: 4px;
    padding: 8px;
  }

  .model-mini small,
  .muted-copy {
    color: var(--dim);
    font-size: 12px;
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
  .compute-block,
  .control-band,
  .distribution-panel,
  .schema-editor > div,
  .rag-grid article,
  .agent-node,
  .api-dashboard,
  .log-console,
  .settings-grid article {
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
    border-radius: 999px;
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
    overflow: hidden;
  }

  .table-row {
    display: grid;
    grid-template-columns: minmax(0, 2fr) 100px 120px 120px 120px;
    gap: 12px;
    align-items: center;
    min-height: 42px;
    padding: 0 12px;
    border-bottom: 1px solid var(--border);
    font-size: 13px;
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
  .rag-grid,
  .settings-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 12px;
  }

  .compute-block {
    min-height: 190px;
    padding: 14px;
  }

  .block-top,
  .stat-row,
  .context-readout {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .compute-block h2 {
    margin: 18px 0 8px;
    font-size: 15px;
  }

  .compute-block p {
    min-height: 36px;
    margin: 0 0 16px;
    color: var(--muted);
    font-size: 12px;
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

  .control-band > div,
  .tuning-control {
    display: grid;
    gap: 8px;
  }

  input[type="range"] {
    height: 4px;
    padding: 0;
    accent-color: var(--amber);
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
    padding: 14px;
  }

  .distribution-chart {
    height: 220px;
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

  .rag-grid article {
    min-height: 300px;
    padding: 14px;
  }

  .retrieval-dock,
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

  .agent-node {
    min-height: 150px;
    display: grid;
    align-content: center;
    justify-items: center;
    gap: 10px;
    padding: 16px;
    text-align: center;
  }

  .agent-node span {
    color: var(--muted);
    font-size: 12px;
  }

  .tool-permissions {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 10px;
  }

  .tool-permissions label {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--panel);
  }

  .tool-permissions label.danger {
    border-color: color-mix(in srgb, var(--red) 30%, var(--border));
  }

  .api-dashboard {
    display: grid;
    gap: 12px;
    padding: 14px;
  }

  .api-url {
    grid-template-columns: 90px minmax(0, 1fr) 34px;
  }

  .api-url button {
    height: 30px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--panel-2);
    color: var(--text);
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

  .settings-grid article {
    min-height: 120px;
    display: grid;
    align-content: start;
    gap: 8px;
    padding: 14px;
  }

  .settings-grid small {
    color: var(--muted);
    line-height: 1.5;
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
    background: rgba(0, 0, 0, 0.24);
    cursor: default;
  }

  .command-palette {
    position: fixed;
    top: 64px;
    left: 50%;
    width: min(640px, calc(100vw - 80px));
    transform: translateX(-50%);
    overflow: hidden;
    border: 1px solid var(--border-strong);
    border-radius: 8px;
    background: var(--panel);
    box-shadow: 0 22px 80px rgba(0, 0, 0, 0.4);
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
    min-height: 36px;
    display: flex;
    align-items: center;
    gap: 10px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
  }

  .palette-results button:hover {
    color: var(--text);
    background: var(--panel-2);
  }

  @media (max-width: 1200px) {
    .shell {
      grid-template-columns: 56px 240px minmax(0, 1fr) 260px;
    }

    .blueprint-grid,
    .rag-grid,
    .agent-canvas,
    .settings-grid,
    .tool-permissions {
      grid-template-columns: 1fr;
    }

    .tuning-grid,
    .schema-editor {
      grid-template-columns: 1fr;
    }
  }
</style>
