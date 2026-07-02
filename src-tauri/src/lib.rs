use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::{
    collections::HashMap,
    env, fs,
    fs::File,
    io::{Read, Seek, SeekFrom},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command as ProcessCommand, Stdio},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use sysinfo::System;
use tauri::{AppHandle, Manager, State};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const PROFILE_EXTENSION: &str = "kivarro.json";
const GGUF_MAGIC: &[u8; 4] = b"GGUF";
const GGUF_VALUE_TYPE_ARRAY: u32 = 9;
const GGUF_VALUE_TYPE_STRING: u32 = 8;
const MAX_GGUF_METADATA_PAIRS: u64 = 50_000;
const MAX_GGUF_KEY_BYTES: u64 = 65_535;
const MAX_GGUF_STRING_BYTES: u64 = 16 * 1024 * 1024;
const MAX_GGUF_ARRAY_ITEMS: u64 = 2_000_000;
const MAX_GGUF_ARRAY_DEPTH: u8 = 4;
const DEFAULT_API_HOST: &str = "127.0.0.1";
const DEFAULT_API_PORT: u16 = 8080;
const LLAMA_SERVER_ENV: &str = "KIVARRO_LLAMA_SERVER";
const API_PORT_ENV: &str = "KIVARRO_API_PORT";
const MAX_HTTP_RESPONSE_BYTES: u64 = 16 * 1024 * 1024;
const HTTP_HEALTH_TIMEOUT_MS: u64 = 400;
const HTTP_CHAT_TIMEOUT_MS: u64 = 3_600_000;

#[cfg(target_os = "windows")]
const WINDOWS_CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MemorySegment {
    label: String,
    gib: f64,
    color: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ComputeBlock {
    id: String,
    name: String,
    kind: String,
    status: String,
    utilization_percent: f32,
    memory_total_gib: Option<f64>,
    memory_used_gib: Option<f64>,
    segments: Vec<MemorySegment>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HardwareSnapshot {
    os: String,
    architecture: String,
    cpu_brand: String,
    cpu_cores: usize,
    cpu_utilization_percent: f32,
    ram_total_gib: f64,
    ram_used_gib: f64,
    blocks: Vec<ComputeBlock>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeMetrics {
    active_model: String,
    active_backend: String,
    server_url: String,
    api_port: u16,
    api_online: bool,
    tokens_per_second: f32,
    context_used_tokens: u32,
    context_total_tokens: u32,
    cpu_utilization_percent: f32,
    gpu_utilization_percent: f32,
    ram_used_gib: f64,
    ram_total_gib: f64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ModelRecord {
    id: String,
    name: String,
    path: String,
    format: String,
    size_gib: f64,
    status: String,
    fit: String,
    architecture: Option<String>,
    parameter_size: Option<String>,
    quantization: Option<String>,
    context_length: Option<u32>,
    block_count: Option<u16>,
    tensor_count: Option<u64>,
    gguf_version: Option<u32>,
    metadata_source: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SamplingParameters {
    temperature: f32,
    top_p: f32,
    top_k: u32,
    min_p: f32,
    typical_p: f32,
    repeat_penalty: f32,
    repeat_last_n: i32,
    presence_penalty: f32,
    frequency_penalty: f32,
    mirostat_mode: u8,
    mirostat_tau: f32,
    mirostat_eta: f32,
    seed: Option<i64>,
    max_tokens: u32,
    stop_sequences: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct RuntimeParameters {
    backend: String,
    context_length: u32,
    batch_size: u32,
    micro_batch_size: u32,
    cpu_threads: u16,
    gpu_layers: u16,
    tensor_split: Vec<f32>,
    main_gpu: Option<u16>,
    use_mmap: bool,
    use_mlock: bool,
    flash_attention: bool,
    kv_cache_quantization: String,
    rope_frequency_base: Option<f32>,
    rope_frequency_scale: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct OutputConstraints {
    mode: String,
    json_schema: String,
    grammar: String,
    logit_bias: Vec<LogitBiasEntry>,
    logprobs: bool,
    top_logprobs: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LogitBiasEntry {
    token: String,
    bias: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct InferenceProfile {
    id: String,
    name: String,
    description: String,
    system_prompt: String,
    sampling: SamplingParameters,
    runtime: RuntimeParameters,
    output: OutputConstraints,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LoadPlanSegment {
    label: String,
    gib: f64,
    color: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelLoadPlan {
    model_id: String,
    profile_id: String,
    backend: String,
    fit: String,
    recommendation: String,
    model_name: String,
    architecture: Option<String>,
    parameter_size: Option<String>,
    quantization: Option<String>,
    model_context_length: Option<u32>,
    metadata_source: String,
    estimated_layers: u16,
    gpu_layers: u16,
    cpu_layers: u16,
    model_weights_gib: f64,
    kv_cache_gib: f64,
    runtime_overhead_gib: f64,
    total_required_gib: f64,
    ram_total_gib: f64,
    ram_available_gib: f64,
    segments: Vec<LoadPlanSegment>,
}

#[derive(Debug, Clone, Default)]
struct GgufMetadataSummary {
    version: u32,
    tensor_count: u64,
    name: Option<String>,
    basename: Option<String>,
    architecture: Option<String>,
    parameter_size: Option<String>,
    quantization: Option<String>,
    context_length: Option<u32>,
    block_count: Option<u16>,
}

#[derive(Debug, Clone)]
enum GgufScalarValue {
    String(String),
    Unsigned(u64),
    Signed(i64),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiEndpoint {
    method: String,
    path: String,
    description: String,
    status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiStatus {
    enabled: bool,
    port: u16,
    base_url: String,
    endpoints: Vec<ApiEndpoint>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BenchmarkResult {
    model: String,
    backend: String,
    eval_count: u32,
    eval_duration_ms: u64,
    tokens_per_second: f32,
    load_duration_ms: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LogEntry {
    level: String,
    source: String,
    message: String,
    timestamp: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct EngineStatus {
    backend: String,
    state: String,
    message: String,
    configured: bool,
    binary_path: Option<String>,
    pid: Option<u32>,
    active_model_id: Option<String>,
    active_model_name: Option<String>,
    host: String,
    port: u16,
    base_url: String,
    health_ok: bool,
    last_tokens_per_second: f32,
    context_used_tokens: u32,
    context_total_tokens: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InferenceRunResult {
    content: String,
    model: String,
    backend: String,
    elapsed_ms: u128,
    tokens_per_second: f32,
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
    total_tokens: Option<u32>,
    finish_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatTurn {
    role: String,
    content: String,
}

#[derive(Debug)]
struct HttpResponse {
    status_code: u16,
    body: Vec<u8>,
}

struct EngineRuntime {
    inner: Mutex<ManagedEngine>,
}

struct ManagedEngine {
    child: Option<Child>,
    active_model_id: Option<String>,
    active_model_name: Option<String>,
    host: String,
    port: u16,
    last_error: Option<String>,
    last_tokens_per_second: f32,
    context_used_tokens: u32,
    context_total_tokens: u32,
}

impl Default for EngineRuntime {
    fn default() -> Self {
        Self {
            inner: Mutex::new(ManagedEngine::default()),
        }
    }
}

impl Default for ManagedEngine {
    fn default() -> Self {
        Self {
            child: None,
            active_model_id: None,
            active_model_name: None,
            host: DEFAULT_API_HOST.to_string(),
            port: configured_api_port(),
            last_error: None,
            last_tokens_per_second: 0.0,
            context_used_tokens: 0,
            context_total_tokens: 32768,
        }
    }
}

#[tauri::command]
fn get_hardware_snapshot() -> Result<HardwareSnapshot, String> {
    let mut system = System::new_all();
    system.refresh_all();

    let ram_total_gib = bytes_to_gib(system.total_memory());
    let ram_used_gib = bytes_to_gib(system.used_memory());
    let cpu_brand = system
        .cpus()
        .first()
        .map(|cpu| cpu.brand().trim().to_string())
        .filter(|brand| !brand.is_empty())
        .unwrap_or_else(|| "Unknown CPU".to_string());
    let cpu_utilization_percent = system.global_cpu_usage();

    let ram_segments = vec![
        MemorySegment {
            label: "Used".to_string(),
            gib: ram_used_gib,
            color: "amber".to_string(),
        },
        MemorySegment {
            label: "Available".to_string(),
            gib: (ram_total_gib - ram_used_gib).max(0.0),
            color: "cyan".to_string(),
        },
    ];

    Ok(HardwareSnapshot {
        os: env::consts::OS.to_string(),
        architecture: env::consts::ARCH.to_string(),
        cpu_brand: cpu_brand.clone(),
        cpu_cores: system.cpus().len(),
        cpu_utilization_percent,
        ram_total_gib,
        ram_used_gib,
        blocks: vec![
            ComputeBlock {
                id: "cpu".to_string(),
                name: cpu_brand,
                kind: "CPU".to_string(),
                status: "Ready".to_string(),
                utilization_percent: cpu_utilization_percent,
                memory_total_gib: None,
                memory_used_gib: None,
                segments: Vec::new(),
            },
            ComputeBlock {
                id: "ram".to_string(),
                name: "System Memory".to_string(),
                kind: "RAM".to_string(),
                status: "Telemetry active".to_string(),
                utilization_percent: percent(ram_used_gib, ram_total_gib),
                memory_total_gib: Some(ram_total_gib),
                memory_used_gib: Some(ram_used_gib),
                segments: ram_segments,
            },
            ComputeBlock {
                id: "gpu-probe".to_string(),
                name: "Accelerator Probe".to_string(),
                kind: "GPU".to_string(),
                status: "GPU telemetry adapter not connected yet".to_string(),
                utilization_percent: 0.0,
                memory_total_gib: None,
                memory_used_gib: None,
                segments: Vec::new(),
            },
        ],
    })
}

#[tauri::command]
fn get_runtime_metrics(engine: State<'_, EngineRuntime>) -> Result<RuntimeMetrics, String> {
    let snapshot = get_hardware_snapshot()?;
    let status = current_engine_status(&engine)?;

    Ok(RuntimeMetrics {
        active_model: status
            .active_model_name
            .clone()
            .unwrap_or_else(|| "No model loaded".to_string()),
        active_backend: format!("{} / {}", status.backend, status.state),
        server_url: status.base_url,
        api_port: status.port,
        api_online: status.health_ok,
        tokens_per_second: status.last_tokens_per_second,
        context_used_tokens: status.context_used_tokens,
        context_total_tokens: status.context_total_tokens,
        cpu_utilization_percent: snapshot.cpu_utilization_percent,
        gpu_utilization_percent: 0.0,
        ram_used_gib: snapshot.ram_used_gib,
        ram_total_gib: snapshot.ram_total_gib,
    })
}

#[tauri::command]
fn list_models() -> Result<Vec<ModelRecord>, String> {
    let mut models = Vec::new();
    for directory in model_search_paths()? {
        collect_models(&directory, &mut models)?;
    }

    models.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(models)
}

#[tauri::command]
fn list_inference_profiles(app_handle: AppHandle) -> Result<Vec<InferenceProfile>, String> {
    let directory = profile_directory(&app_handle)?;
    fs::create_dir_all(&directory).map_err(|err| {
        format!(
            "failed to create profile directory {}: {err}",
            directory.display()
        )
    })?;

    seed_default_profiles(&directory)?;

    let mut profiles = Vec::new();
    for entry in fs::read_dir(&directory).map_err(|err| {
        format!(
            "failed to read profile directory {}: {err}",
            directory.display()
        )
    })? {
        let entry = entry.map_err(|err| format!("failed to read profile entry: {err}"))?;
        let path = entry.path();

        if !is_profile_file(&path) {
            continue;
        }

        let raw = fs::read_to_string(&path)
            .map_err(|err| format!("failed to read profile {}: {err}", path.display()))?;
        let profile = serde_json::from_str::<InferenceProfile>(&raw)
            .map_err(|err| format!("failed to parse profile {}: {err}", path.display()))?;

        profiles.push(profile);
    }

    profiles.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(profiles)
}

#[tauri::command]
fn save_inference_profile(
    app_handle: AppHandle,
    profile: InferenceProfile,
) -> Result<InferenceProfile, String> {
    let directory = profile_directory(&app_handle)?;
    fs::create_dir_all(&directory).map_err(|err| {
        format!(
            "failed to create profile directory {}: {err}",
            directory.display()
        )
    })?;

    let profile = normalize_profile(profile);
    validate_profile(&profile)?;

    let path = directory.join(format!("{}.{}", profile.id, PROFILE_EXTENSION));
    let encoded = serde_json::to_string_pretty(&profile)
        .map_err(|err| format!("failed to encode profile {}: {err}", profile.id))?;

    fs::write(&path, encoded)
        .map_err(|err| format!("failed to write profile {}: {err}", path.display()))?;

    Ok(profile)
}

#[tauri::command]
fn delete_inference_profile(app_handle: AppHandle, id: String) -> Result<(), String> {
    let id = sanitize_identifier(&id);
    if id.is_empty() {
        return Err("profile id is required".to_string());
    }

    let path = profile_directory(&app_handle)?.join(format!("{id}.{PROFILE_EXTENSION}"));
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|err| format!("failed to delete profile {}: {err}", path.display()))?;
    }

    Ok(())
}

#[tauri::command]
fn get_model_load_plan(
    model_id: String,
    profile: InferenceProfile,
) -> Result<ModelLoadPlan, String> {
    validate_profile(&profile)?;

    let model_path = PathBuf::from(&model_id);
    if !model_path.exists() {
        return Err(format!("model does not exist: {}", model_path.display()));
    }
    if !is_supported_model_file(&model_path) {
        return Err(format!(
            "unsupported model file extension: {}",
            model_path.display()
        ));
    }

    let metadata = fs::metadata(&model_path).map_err(|err| {
        format!(
            "failed to read model metadata {}: {err}",
            model_path.display()
        )
    })?;
    let model_weights_gib = bytes_to_gib(metadata.len());
    let fallback_model_name = model_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("model");
    let gguf_metadata = read_model_gguf_metadata(&model_path);
    let model_name = gguf_metadata
        .as_ref()
        .map(|summary| gguf_display_name(summary, fallback_model_name))
        .unwrap_or_else(|| fallback_model_name.to_string());
    let architecture = gguf_metadata
        .as_ref()
        .and_then(|summary| summary.architecture.clone());
    let parameter_size = gguf_metadata
        .as_ref()
        .and_then(|summary| summary.parameter_size.clone());
    let quantization = gguf_metadata
        .as_ref()
        .and_then(|summary| summary.quantization.clone())
        .or_else(|| infer_quantization_from_name(fallback_model_name));
    let model_context_length = gguf_metadata
        .as_ref()
        .and_then(|summary| summary.context_length);
    let block_count = gguf_metadata
        .as_ref()
        .and_then(|summary| summary.block_count);
    let metadata_source = gguf_metadata
        .as_ref()
        .map(|summary| format!("GGUF v{}", summary.version))
        .unwrap_or_else(|| "filename".to_string());
    let estimated_layers = estimate_layer_count(&model_name, block_count);
    let gpu_layers = profile.runtime.gpu_layers.min(estimated_layers);
    let cpu_layers = estimated_layers.saturating_sub(gpu_layers);
    let kv_cache_gib = estimate_kv_cache_gib(
        profile.runtime.context_length,
        estimated_layers,
        &profile.runtime.kv_cache_quantization,
    );
    let runtime_overhead_gib = round_gib((model_weights_gib * 0.08).max(0.6));
    let total_required_gib = round_gib(model_weights_gib + kv_cache_gib + runtime_overhead_gib);

    let mut system = System::new_all();
    system.refresh_memory();
    let ram_total_gib = bytes_to_gib(system.total_memory());
    let ram_used_gib = bytes_to_gib(system.used_memory());
    let ram_available_gib = (ram_total_gib - ram_used_gib).max(0.0);
    let fit = if total_required_gib <= ram_available_gib {
        "Fits available RAM"
    } else if total_required_gib <= ram_total_gib {
        "Fits with memory pressure"
    } else {
        "Exceeds system RAM"
    }
    .to_string();

    let recommendation = if let Some(model_context_length) = model_context_length
        .filter(|context_length| profile.runtime.context_length > *context_length)
    {
        format!(
            "Requested {} token context exceeds GGUF context {}; reduce context length or verify RoPE scaling before loading.",
            profile.runtime.context_length, model_context_length
        )
    } else if fit == "Fits available RAM" && gpu_layers == estimated_layers {
        "All estimated layers can stay on the selected accelerator profile.".to_string()
    } else if fit == "Fits available RAM" {
        format!("{cpu_layers} estimated layers remain on CPU; increase GPU layers if VRAM allows.")
    } else if fit == "Fits with memory pressure" {
        "Reduce context length or KV cache precision before loading under memory pressure."
            .to_string()
    } else {
        "Choose a smaller quantization, lower context length, or a smaller model.".to_string()
    };

    Ok(ModelLoadPlan {
        model_id,
        profile_id: profile.id,
        backend: profile.runtime.backend,
        fit,
        recommendation,
        model_name,
        architecture,
        parameter_size,
        quantization,
        model_context_length,
        metadata_source,
        estimated_layers,
        gpu_layers,
        cpu_layers,
        model_weights_gib,
        kv_cache_gib,
        runtime_overhead_gib,
        total_required_gib,
        ram_total_gib,
        ram_available_gib,
        segments: vec![
            LoadPlanSegment {
                label: "Model weights".to_string(),
                gib: model_weights_gib,
                color: "amber".to_string(),
            },
            LoadPlanSegment {
                label: "KV cache".to_string(),
                gib: kv_cache_gib,
                color: "cyan".to_string(),
            },
            LoadPlanSegment {
                label: "Runtime overhead".to_string(),
                gib: runtime_overhead_gib,
                color: "magenta".to_string(),
            },
            LoadPlanSegment {
                label: "Available RAM after current use".to_string(),
                gib: ram_available_gib,
                color: "green".to_string(),
            },
        ],
    })
}

#[tauri::command]
fn get_engine_status(engine: State<'_, EngineRuntime>) -> Result<EngineStatus, String> {
    current_engine_status(&engine)
}

#[tauri::command]
fn start_llama_server(
    engine: State<'_, EngineRuntime>,
    model_id: String,
    profile: InferenceProfile,
) -> Result<EngineStatus, String> {
    validate_profile(&profile)?;

    let model_path = canonical_model_path(&model_id)?;
    if !is_gguf_file(&model_path) {
        return Err("llama-server requires a GGUF model for this adapter slice".to_string());
    }
    let canonical_model_id = model_path.to_string_lossy().to_string();

    let binary_path = find_llama_server_binary()
        .ok_or_else(|| format!("llama-server not found. Set {LLAMA_SERVER_ENV} to the binary path or add llama-server to PATH."))?;
    let model_name = model_display_name_from_path(&model_path);
    let port = configured_api_port();
    let host = DEFAULT_API_HOST.to_string();
    let args = build_llama_server_args(&model_path, &profile, &host, port);

    let mut guard = engine
        .inner
        .lock()
        .map_err(|_| "engine state lock is poisoned".to_string())?;

    refresh_engine_process(&mut guard);
    if guard.child.is_some()
        && guard.active_model_id.as_deref() == Some(canonical_model_id.as_str())
    {
        guard.last_error = None;
        return Ok(engine_status_from_guard(&mut guard, Some(binary_path)));
    }

    stop_child(&mut guard)?;

    let mut command = ProcessCommand::new(&binary_path);
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(target_os = "windows")]
    command.creation_flags(WINDOWS_CREATE_NO_WINDOW);

    let child = command.spawn().map_err(|err| {
        format!(
            "failed to start llama-server at {}: {err}",
            binary_path.display()
        )
    })?;

    guard.child = Some(child);
    guard.active_model_id = Some(canonical_model_id);
    guard.active_model_name = Some(model_name);
    guard.host = host;
    guard.port = port;
    guard.last_error = None;
    guard.last_tokens_per_second = 0.0;
    guard.context_used_tokens = 0;
    guard.context_total_tokens = profile.runtime.context_length;

    Ok(engine_status_from_guard(&mut guard, Some(binary_path)))
}

#[tauri::command]
fn stop_llama_server(engine: State<'_, EngineRuntime>) -> Result<EngineStatus, String> {
    let mut guard = engine
        .inner
        .lock()
        .map_err(|_| "engine state lock is poisoned".to_string())?;

    stop_child(&mut guard)?;
    guard.active_model_id = None;
    guard.active_model_name = None;
    guard.last_error = None;
    guard.last_tokens_per_second = 0.0;
    guard.context_used_tokens = 0;

    Ok(engine_status_from_guard(
        &mut guard,
        find_llama_server_binary(),
    ))
}

#[tauri::command]
fn run_chat_completion(
    engine: State<'_, EngineRuntime>,
    model_id: String,
    profile: InferenceProfile,
    prompt: String,
    history: Vec<ChatTurn>,
) -> Result<InferenceRunResult, String> {
    validate_profile(&profile)?;
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return Err("prompt is required".to_string());
    }
    let requested_model_id = canonical_model_path(&model_id)?
        .to_string_lossy()
        .to_string();

    let (host, port, active_model_id, active_model_name) = {
        let mut guard = engine
            .inner
            .lock()
            .map_err(|_| "engine state lock is poisoned".to_string())?;
        refresh_engine_process(&mut guard);

        let active_model_id = guard
            .active_model_id
            .clone()
            .ok_or_else(|| "no llama-server model is loaded".to_string())?;
        if active_model_id != requested_model_id {
            return Err("selected model is not the active llama-server model".to_string());
        }
        if guard.child.is_none() {
            return Err("llama-server is not running".to_string());
        }

        (
            guard.host.clone(),
            guard.port,
            active_model_id,
            guard
                .active_model_name
                .clone()
                .unwrap_or_else(|| "local-model".to_string()),
        )
    };

    match probe_llama_health(&host, port, HTTP_HEALTH_TIMEOUT_MS) {
        HealthProbe::Ready => {}
        HealthProbe::Loading(message) => {
            return Err(format!("llama-server is still loading: {message}"));
        }
        HealthProbe::Offline(message) => {
            return Err(format!("llama-server is not reachable: {message}"));
        }
    }

    let started = SystemTime::now();
    let payload = build_chat_completion_payload(&profile, &active_model_name, prompt, &history)?;
    let response = http_json_request(
        &host,
        port,
        "POST",
        "/v1/chat/completions",
        Some(&payload),
        HTTP_CHAT_TIMEOUT_MS,
    )?;
    let elapsed_ms = started
        .elapsed()
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or_default();
    let body = parse_llama_json_response(response)?;
    let content = body
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .or_else(|| body.pointer("/choices/0/text").and_then(Value::as_str))
        .unwrap_or_default()
        .to_string();

    if content.is_empty() {
        return Err("llama-server returned an empty completion".to_string());
    }

    let prompt_tokens = json_u32(&body, "/usage/prompt_tokens");
    let completion_tokens = json_u32(&body, "/usage/completion_tokens");
    let total_tokens = json_u32(&body, "/usage/total_tokens");
    let finish_reason = body
        .pointer("/choices/0/finish_reason")
        .and_then(Value::as_str)
        .map(str::to_string);
    let tokens_per_second = completion_tokens
        .filter(|_| elapsed_ms > 0)
        .map(|tokens| (tokens as f64 / elapsed_ms as f64 * 1000.0) as f32)
        .unwrap_or(0.0);

    {
        let mut guard = engine
            .inner
            .lock()
            .map_err(|_| "engine state lock is poisoned".to_string())?;
        guard.last_tokens_per_second = tokens_per_second;
        guard.context_used_tokens = total_tokens.unwrap_or_else(|| {
            (prompt_tokens.unwrap_or_default() + completion_tokens.unwrap_or_default())
                .min(profile.runtime.context_length)
        });
        guard.context_total_tokens = profile.runtime.context_length;
        guard.active_model_id = Some(active_model_id);
        guard.active_model_name = Some(active_model_name.clone());
        guard.last_error = None;
    }

    Ok(InferenceRunResult {
        content,
        model: active_model_name,
        backend: "llama.cpp".to_string(),
        elapsed_ms,
        tokens_per_second,
        prompt_tokens,
        completion_tokens,
        total_tokens,
        finish_reason,
    })
}

#[tauri::command]
fn get_api_status(engine: State<'_, EngineRuntime>) -> Result<ApiStatus, String> {
    let status = current_engine_status(&engine)?;

    Ok(ApiStatus {
        enabled: matches!(status.state.as_str(), "ready" | "loading"),
        port: status.port,
        base_url: status.base_url,
        endpoints: vec![
            ApiEndpoint {
                method: "POST".to_string(),
                path: "/v1/chat/completions".to_string(),
                description: "OpenAI-compatible local chat completions through llama-server"
                    .to_string(),
                status: if status.health_ok { "Ready" } else { "Offline" }.to_string(),
            },
            ApiEndpoint {
                method: "POST".to_string(),
                path: "/v1/embeddings".to_string(),
                description: "Local embedding generation for RAG pipelines".to_string(),
                status: "Planned".to_string(),
            },
            ApiEndpoint {
                method: "GET".to_string(),
                path: "/v1/models".to_string(),
                description: "Loaded and discoverable local models".to_string(),
                status: if status.health_ok { "Ready" } else { "Offline" }.to_string(),
            },
            ApiEndpoint {
                method: "GET".to_string(),
                path: "/health".to_string(),
                description: "llama-server readiness probe".to_string(),
                status: status.state,
            },
        ],
    })
}

#[tauri::command]
fn list_benchmark_results() -> Result<Vec<BenchmarkResult>, String> {
    Ok(Vec::new())
}

#[tauri::command]
fn list_system_logs() -> Result<Vec<LogEntry>, String> {
    Ok(vec![
        LogEntry {
            level: "INFO".to_string(),
            source: "core".to_string(),
            message: "Kivarro desktop shell initialized".to_string(),
            timestamp: "local session".to_string(),
        },
        LogEntry {
            level: "INFO".to_string(),
            source: "registry".to_string(),
            message: "Model discovery scans the local ./models directory in this baseline"
                .to_string(),
            timestamp: "local session".to_string(),
        },
        LogEntry {
            level: "WARN".to_string(),
            source: "accelerator".to_string(),
            message: "GPU telemetry is reserved for the hardware adapter layer".to_string(),
            timestamp: "local session".to_string(),
        },
    ])
}

fn profile_directory(app_handle: &AppHandle) -> Result<PathBuf, String> {
    app_handle
        .path()
        .app_config_dir()
        .map(|path| path.join("profiles"))
        .map_err(|err| format!("failed to resolve app profile directory: {err}"))
}

fn seed_default_profiles(directory: &Path) -> Result<(), String> {
    let has_profiles = fs::read_dir(directory)
        .map_err(|err| {
            format!(
                "failed to inspect profile directory {}: {err}",
                directory.display()
            )
        })?
        .filter_map(Result::ok)
        .any(|entry| is_profile_file(&entry.path()));

    if has_profiles {
        return Ok(());
    }

    for profile in default_profiles() {
        let path = directory.join(format!("{}.{}", profile.id, PROFILE_EXTENSION));
        let encoded = serde_json::to_string_pretty(&profile)
            .map_err(|err| format!("failed to encode default profile {}: {err}", profile.id))?;
        fs::write(&path, encoded)
            .map_err(|err| format!("failed to seed profile {}: {err}", path.display()))?;
    }

    Ok(())
}

fn default_profiles() -> Vec<InferenceProfile> {
    vec![
        InferenceProfile {
            id: "balanced-engineer".to_string(),
            name: "Balanced Engineer".to_string(),
            description: "General technical work with stable sampling and long-context defaults."
                .to_string(),
            system_prompt: "You are a precise local AI assistant. Prefer concise, verifiable answers and surface uncertainty explicitly.".to_string(),
            sampling: SamplingParameters {
                temperature: 0.7,
                top_p: 0.92,
                top_k: 40,
                min_p: 0.05,
                typical_p: 1.0,
                repeat_penalty: 1.1,
                repeat_last_n: 256,
                presence_penalty: 0.0,
                frequency_penalty: 0.0,
                mirostat_mode: 0,
                mirostat_tau: 5.0,
                mirostat_eta: 0.1,
                seed: None,
                max_tokens: 2048,
                stop_sequences: Vec::new(),
            },
            runtime: RuntimeParameters {
                backend: "llama.cpp".to_string(),
                context_length: 32768,
                batch_size: 512,
                micro_batch_size: 128,
                cpu_threads: default_thread_count(),
                gpu_layers: 0,
                tensor_split: Vec::new(),
                main_gpu: None,
                use_mmap: true,
                use_mlock: false,
                flash_attention: true,
                kv_cache_quantization: "f16".to_string(),
                rope_frequency_base: None,
                rope_frequency_scale: None,
            },
            output: OutputConstraints {
                mode: "text".to_string(),
                json_schema: String::new(),
                grammar: String::new(),
                logit_bias: Vec::new(),
                logprobs: false,
                top_logprobs: 0,
            },
            created_at: "built-in".to_string(),
            updated_at: "built-in".to_string(),
        },
        InferenceProfile {
            id: "strict-json-extractor".to_string(),
            name: "Strict JSON Extractor".to_string(),
            description: "Low-temperature extraction profile with JSON schema constraints ready."
                .to_string(),
            system_prompt:
                "Return only valid JSON that satisfies the active schema. Do not include prose."
                    .to_string(),
            sampling: SamplingParameters {
                temperature: 0.1,
                top_p: 0.85,
                top_k: 20,
                min_p: 0.01,
                typical_p: 1.0,
                repeat_penalty: 1.05,
                repeat_last_n: 128,
                presence_penalty: 0.0,
                frequency_penalty: 0.0,
                mirostat_mode: 0,
                mirostat_tau: 5.0,
                mirostat_eta: 0.1,
                seed: Some(42),
                max_tokens: 2048,
                stop_sequences: Vec::new(),
            },
            runtime: RuntimeParameters {
                backend: "llama.cpp".to_string(),
                context_length: 16384,
                batch_size: 512,
                micro_batch_size: 128,
                cpu_threads: default_thread_count(),
                gpu_layers: 0,
                tensor_split: Vec::new(),
                main_gpu: None,
                use_mmap: true,
                use_mlock: false,
                flash_attention: true,
                kv_cache_quantization: "f16".to_string(),
                rope_frequency_base: None,
                rope_frequency_scale: None,
            },
            output: OutputConstraints {
                mode: "json_schema".to_string(),
                json_schema: "{\n  \"type\": \"object\",\n  \"properties\": {},\n  \"additionalProperties\": true\n}".to_string(),
                grammar: String::new(),
                logit_bias: Vec::new(),
                logprobs: false,
                top_logprobs: 0,
            },
            created_at: "built-in".to_string(),
            updated_at: "built-in".to_string(),
        },
        InferenceProfile {
            id: "local-code-reviewer".to_string(),
            name: "Local Code Reviewer".to_string(),
            description: "Deterministic review profile tuned for code, diffs, and concrete findings."
                .to_string(),
            system_prompt:
                "Review code for correctness, regressions, security issues, and missing tests. Lead with actionable findings."
                    .to_string(),
            sampling: SamplingParameters {
                temperature: 0.25,
                top_p: 0.9,
                top_k: 40,
                min_p: 0.03,
                typical_p: 1.0,
                repeat_penalty: 1.08,
                repeat_last_n: 256,
                presence_penalty: 0.0,
                frequency_penalty: 0.0,
                mirostat_mode: 0,
                mirostat_tau: 5.0,
                mirostat_eta: 0.1,
                seed: None,
                max_tokens: 4096,
                stop_sequences: Vec::new(),
            },
            runtime: RuntimeParameters {
                backend: "llama.cpp".to_string(),
                context_length: 65536,
                batch_size: 1024,
                micro_batch_size: 256,
                cpu_threads: default_thread_count(),
                gpu_layers: 0,
                tensor_split: Vec::new(),
                main_gpu: None,
                use_mmap: true,
                use_mlock: false,
                flash_attention: true,
                kv_cache_quantization: "q8_0".to_string(),
                rope_frequency_base: None,
                rope_frequency_scale: None,
            },
            output: OutputConstraints {
                mode: "text".to_string(),
                json_schema: String::new(),
                grammar: String::new(),
                logit_bias: Vec::new(),
                logprobs: true,
                top_logprobs: 5,
            },
            created_at: "built-in".to_string(),
            updated_at: "built-in".to_string(),
        },
        InferenceProfile {
            id: "long-context-analyst".to_string(),
            name: "Long Context Analyst".to_string(),
            description: "Large-context analysis with compressed KV cache and conservative decoding."
                .to_string(),
            system_prompt:
                "Analyze long context carefully. Track assumptions, cite relevant sections, and avoid inventing missing facts."
                    .to_string(),
            sampling: SamplingParameters {
                temperature: 0.35,
                top_p: 0.9,
                top_k: 30,
                min_p: 0.02,
                typical_p: 1.0,
                repeat_penalty: 1.12,
                repeat_last_n: 512,
                presence_penalty: 0.0,
                frequency_penalty: 0.1,
                mirostat_mode: 0,
                mirostat_tau: 5.0,
                mirostat_eta: 0.1,
                seed: None,
                max_tokens: 8192,
                stop_sequences: Vec::new(),
            },
            runtime: RuntimeParameters {
                backend: "llama.cpp".to_string(),
                context_length: 131072,
                batch_size: 1024,
                micro_batch_size: 256,
                cpu_threads: default_thread_count(),
                gpu_layers: 0,
                tensor_split: Vec::new(),
                main_gpu: None,
                use_mmap: true,
                use_mlock: false,
                flash_attention: true,
                kv_cache_quantization: "q4_0".to_string(),
                rope_frequency_base: None,
                rope_frequency_scale: None,
            },
            output: OutputConstraints {
                mode: "text".to_string(),
                json_schema: String::new(),
                grammar: String::new(),
                logit_bias: Vec::new(),
                logprobs: false,
                top_logprobs: 0,
            },
            created_at: "built-in".to_string(),
            updated_at: "built-in".to_string(),
        },
    ]
}

fn normalize_profile(mut profile: InferenceProfile) -> InferenceProfile {
    profile.id = sanitize_identifier(if profile.id.trim().is_empty() {
        &profile.name
    } else {
        &profile.id
    });

    if profile.id.is_empty() {
        profile.id = format!("profile-{}", unix_timestamp());
    }

    let now = unix_timestamp().to_string();
    if profile.created_at.trim().is_empty() {
        profile.created_at = now.clone();
    }
    profile.updated_at = now;
    profile
}

fn validate_profile(profile: &InferenceProfile) -> Result<(), String> {
    if profile.name.trim().is_empty() {
        return Err("profile name is required".to_string());
    }
    if !(0.0..=2.0).contains(&profile.sampling.temperature) {
        return Err("temperature must be between 0 and 2".to_string());
    }
    if !(0.0..=1.0).contains(&profile.sampling.top_p) {
        return Err("top_p must be between 0 and 1".to_string());
    }
    if !(0.0..=1.0).contains(&profile.sampling.min_p) {
        return Err("min_p must be between 0 and 1".to_string());
    }
    if !(0.0..=2.0).contains(&profile.sampling.repeat_penalty) {
        return Err("repeat penalty must be between 0 and 2".to_string());
    }
    if profile.runtime.context_length < 512 {
        return Err("context length must be at least 512 tokens".to_string());
    }
    if profile.runtime.batch_size == 0 || profile.runtime.micro_batch_size == 0 {
        return Err("batch sizes must be greater than zero".to_string());
    }
    if profile.runtime.cpu_threads == 0 {
        return Err("cpu thread count must be greater than zero".to_string());
    }
    if profile.output.top_logprobs > 20 {
        return Err("top_logprobs must be 20 or lower".to_string());
    }
    Ok(())
}

fn is_profile_file(path: &Path) -> bool {
    path.is_file()
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.ends_with(&format!(".{PROFILE_EXTENSION}")))
            .unwrap_or(false)
}

fn sanitize_identifier(input: &str) -> String {
    input
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|character| match character {
            'a'..='z' | '0'..='9' => character,
            '-' | '_' => character,
            _ => '-',
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn default_thread_count() -> u16 {
    std::thread::available_parallelism()
        .map(|count| count.get().clamp(1, u16::MAX as usize) as u16)
        .unwrap_or(4)
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn configured_api_port() -> u16 {
    env::var(API_PORT_ENV)
        .ok()
        .and_then(|value| value.trim().parse::<u16>().ok())
        .filter(|port| *port > 0)
        .unwrap_or(DEFAULT_API_PORT)
}

fn current_engine_status(engine: &State<'_, EngineRuntime>) -> Result<EngineStatus, String> {
    let binary_path = find_llama_server_binary();
    let mut guard = engine
        .inner
        .lock()
        .map_err(|_| "engine state lock is poisoned".to_string())?;

    Ok(engine_status_from_guard(&mut guard, binary_path))
}

fn engine_status_from_guard(
    guard: &mut ManagedEngine,
    binary_path: Option<PathBuf>,
) -> EngineStatus {
    refresh_engine_process(guard);

    let pid = guard.child.as_ref().map(Child::id);
    let (state, message, health_ok) = if guard.child.is_some() {
        match probe_llama_health(&guard.host, guard.port, HTTP_HEALTH_TIMEOUT_MS) {
            HealthProbe::Ready => (
                "ready".to_string(),
                "llama-server is ready for local chat completions".to_string(),
                true,
            ),
            HealthProbe::Loading(message) => ("loading".to_string(), message, false),
            HealthProbe::Offline(message) => ("loading".to_string(), message, false),
        }
    } else if let Some(error) = guard.last_error.as_ref() {
        ("error".to_string(), error.clone(), false)
    } else if binary_path.is_some() {
        (
            "offline".to_string(),
            "llama-server is configured but no model is loaded".to_string(),
            false,
        )
    } else {
        (
            "unconfigured".to_string(),
            format!("Set {LLAMA_SERVER_ENV} or add llama-server to PATH"),
            false,
        )
    };

    EngineStatus {
        backend: "llama.cpp".to_string(),
        state,
        message,
        configured: binary_path.is_some(),
        binary_path: binary_path.map(|path| path.to_string_lossy().to_string()),
        pid,
        active_model_id: guard.active_model_id.clone(),
        active_model_name: guard.active_model_name.clone(),
        host: guard.host.clone(),
        port: guard.port,
        base_url: format!("http://{}:{}/v1", guard.host, guard.port),
        health_ok,
        last_tokens_per_second: guard.last_tokens_per_second,
        context_used_tokens: guard.context_used_tokens,
        context_total_tokens: guard.context_total_tokens,
    }
}

fn refresh_engine_process(guard: &mut ManagedEngine) {
    let Some(child) = guard.child.as_mut() else {
        return;
    };

    match child.try_wait() {
        Ok(Some(status)) => {
            guard.child = None;
            guard.last_error = Some(format!("llama-server exited with status {status}"));
        }
        Ok(None) => {}
        Err(err) => {
            guard.child = None;
            guard.last_error = Some(format!("failed to inspect llama-server process: {err}"));
        }
    }
}

fn stop_child(guard: &mut ManagedEngine) -> Result<(), String> {
    let Some(mut child) = guard.child.take() else {
        return Ok(());
    };

    match child.try_wait() {
        Ok(Some(_)) => Ok(()),
        Ok(None) => {
            child
                .kill()
                .map_err(|err| format!("failed to stop llama-server: {err}"))?;
            child
                .wait()
                .map(|_| ())
                .map_err(|err| format!("failed to wait for llama-server shutdown: {err}"))
        }
        Err(err) => Err(format!("failed to inspect llama-server before stop: {err}")),
    }
}

fn canonical_model_path(model_id: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(model_id);
    if !path.exists() {
        return Err(format!("model does not exist: {}", path.display()));
    }
    if !is_supported_model_file(&path) {
        return Err(format!(
            "unsupported model file extension: {}",
            path.display()
        ));
    }

    path.canonicalize()
        .map_err(|err| format!("failed to resolve model path {}: {err}", path.display()))
}

fn model_display_name_from_path(path: &Path) -> String {
    read_model_gguf_metadata(path)
        .as_ref()
        .map(|summary| {
            let fallback = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("local-model");
            gguf_display_name(summary, fallback)
        })
        .unwrap_or_else(|| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("local-model")
                .to_string()
        })
}

fn find_llama_server_binary() -> Option<PathBuf> {
    if let Ok(value) = env::var(LLAMA_SERVER_ENV) {
        let trimmed = value.trim().trim_matches('"');
        if !trimmed.is_empty() {
            let configured = PathBuf::from(trimmed);
            if configured.is_file() {
                return Some(configured);
            }
            if let Some(found) = find_executable_on_path(trimmed) {
                return Some(found);
            }
        }
    }

    ["llama-server", "llama-server.exe", "server", "server.exe"]
        .iter()
        .find_map(|candidate| find_executable_on_path(candidate))
}

fn find_executable_on_path(candidate: &str) -> Option<PathBuf> {
    let candidate_path = PathBuf::from(candidate);
    if candidate_path.components().count() > 1 && candidate_path.is_file() {
        return Some(candidate_path);
    }

    let path_var = env::var_os("PATH")?;
    let candidate_names = executable_candidate_names(candidate);
    for directory in env::split_paths(&path_var) {
        for name in &candidate_names {
            let path = directory.join(name);
            if path.is_file() {
                return Some(path);
            }
        }
    }

    None
}

fn executable_candidate_names(candidate: &str) -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        let lower = candidate.to_ascii_lowercase();
        if lower.ends_with(".exe") {
            vec![candidate.to_string()]
        } else {
            vec![candidate.to_string(), format!("{candidate}.exe")]
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        vec![candidate.to_string()]
    }
}

fn build_llama_server_args(
    model_path: &Path,
    profile: &InferenceProfile,
    host: &str,
    port: u16,
) -> Vec<String> {
    let mut args = vec![
        "--model".to_string(),
        model_path.to_string_lossy().to_string(),
        "--host".to_string(),
        host.to_string(),
        "--port".to_string(),
        port.to_string(),
        "--ctx-size".to_string(),
        profile.runtime.context_length.to_string(),
        "--threads".to_string(),
        profile.runtime.cpu_threads.to_string(),
        "--batch-size".to_string(),
        profile.runtime.batch_size.to_string(),
        "--ubatch-size".to_string(),
        profile.runtime.micro_batch_size.to_string(),
        "--gpu-layers".to_string(),
        profile.runtime.gpu_layers.to_string(),
        "--cache-type-k".to_string(),
        profile.runtime.kv_cache_quantization.clone(),
        "--cache-type-v".to_string(),
        profile.runtime.kv_cache_quantization.clone(),
        "--flash-attn".to_string(),
        if profile.runtime.flash_attention {
            "on".to_string()
        } else {
            "off".to_string()
        },
    ];

    if profile.runtime.use_mmap {
        args.push("--mmap".to_string());
    } else {
        args.push("--no-mmap".to_string());
    }

    if profile.runtime.use_mlock {
        args.push("--mlock".to_string());
    }

    if let Some(main_gpu) = profile.runtime.main_gpu {
        args.push("--main-gpu".to_string());
        args.push(main_gpu.to_string());
    }

    if !profile.runtime.tensor_split.is_empty() {
        args.push("--tensor-split".to_string());
        args.push(
            profile
                .runtime
                .tensor_split
                .iter()
                .map(|value| format_float_arg(*value))
                .collect::<Vec<_>>()
                .join(","),
        );
    }

    if let Some(base) = profile.runtime.rope_frequency_base {
        args.push("--rope-freq-base".to_string());
        args.push(format_float_arg(base));
    }

    if let Some(scale) = profile.runtime.rope_frequency_scale {
        args.push("--rope-freq-scale".to_string());
        args.push(format_float_arg(scale));
    }

    args
}

fn format_float_arg(value: f32) -> String {
    let formatted = format!("{value:.6}");
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

enum HealthProbe {
    Ready,
    Loading(String),
    Offline(String),
}

fn probe_llama_health(host: &str, port: u16, timeout_ms: u64) -> HealthProbe {
    match http_json_request(host, port, "GET", "/health", None, timeout_ms) {
        Ok(response) if response.status_code == 200 => HealthProbe::Ready,
        Ok(response) if response.status_code == 503 => {
            let message = serde_json::from_slice::<Value>(&response.body)
                .ok()
                .and_then(|value| {
                    value
                        .pointer("/error/message")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or_else(|| "model is loading".to_string());
            HealthProbe::Loading(message)
        }
        Ok(response) => HealthProbe::Offline(format!(
            "health check returned HTTP {}",
            response.status_code
        )),
        Err(err) => HealthProbe::Offline(err),
    }
}

fn build_chat_completion_payload(
    profile: &InferenceProfile,
    model_name: &str,
    prompt: &str,
    history: &[ChatTurn],
) -> Result<Value, String> {
    let mut messages = Vec::new();
    if !profile.system_prompt.trim().is_empty() {
        messages.push(json!({
            "role": "system",
            "content": profile.system_prompt.trim()
        }));
    }

    for turn in history.iter().take(32) {
        let role = turn.role.trim();
        let content = turn.content.trim();
        if content.is_empty() || !matches!(role, "system" | "user" | "assistant") {
            continue;
        }
        messages.push(json!({ "role": role, "content": content }));
    }

    messages.push(json!({ "role": "user", "content": prompt }));

    let mut payload = Map::new();
    payload.insert("model".to_string(), json!(model_name));
    payload.insert("messages".to_string(), Value::Array(messages));
    payload.insert(
        "temperature".to_string(),
        json!(profile.sampling.temperature),
    );
    payload.insert("top_p".to_string(), json!(profile.sampling.top_p));
    payload.insert("top_k".to_string(), json!(profile.sampling.top_k));
    payload.insert("min_p".to_string(), json!(profile.sampling.min_p));
    payload.insert(
        "repeat_penalty".to_string(),
        json!(profile.sampling.repeat_penalty),
    );
    payload.insert(
        "presence_penalty".to_string(),
        json!(profile.sampling.presence_penalty),
    );
    payload.insert(
        "frequency_penalty".to_string(),
        json!(profile.sampling.frequency_penalty),
    );
    payload.insert("max_tokens".to_string(), json!(profile.sampling.max_tokens));
    payload.insert("stream".to_string(), json!(false));

    if let Some(seed) = profile.sampling.seed {
        payload.insert("seed".to_string(), json!(seed));
    }

    if !profile.sampling.stop_sequences.is_empty() {
        payload.insert("stop".to_string(), json!(profile.sampling.stop_sequences));
    }

    if profile.output.logprobs {
        payload.insert("logprobs".to_string(), json!(true));
        payload.insert(
            "top_logprobs".to_string(),
            json!(profile.output.top_logprobs),
        );
    }

    match profile.output.mode.as_str() {
        "json_schema" if !profile.output.json_schema.trim().is_empty() => {
            let schema = serde_json::from_str::<Value>(&profile.output.json_schema)
                .map_err(|err| format!("profile JSON schema is invalid: {err}"))?;
            payload.insert(
                "response_format".to_string(),
                json!({
                    "type": "json_schema",
                    "json_schema": {
                        "name": "kivarro_profile_schema",
                        "strict": true,
                        "schema": schema
                    }
                }),
            );
        }
        "json" => {
            payload.insert(
                "response_format".to_string(),
                json!({ "type": "json_object" }),
            );
        }
        _ => {}
    }

    if !profile.output.grammar.trim().is_empty() {
        payload.insert("grammar".to_string(), json!(profile.output.grammar.trim()));
    }

    Ok(Value::Object(payload))
}

fn http_json_request(
    host: &str,
    port: u16,
    method: &str,
    path: &str,
    body: Option<&Value>,
    timeout_ms: u64,
) -> Result<HttpResponse, String> {
    let body_bytes = body
        .map(serde_json::to_vec)
        .transpose()
        .map_err(|err| format!("failed to encode request JSON: {err}"))?
        .unwrap_or_default();
    let address = format!("{host}:{port}")
        .parse::<SocketAddr>()
        .map_err(|err| format!("invalid llama-server address {host}:{port}: {err}"))?;
    let timeout = std::time::Duration::from_millis(timeout_ms);
    let mut stream = TcpStream::connect_timeout(&address, timeout)
        .map_err(|err| format!("failed to connect to llama-server at {host}:{port}: {err}"))?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|err| format!("failed to set read timeout: {err}"))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|err| format!("failed to set write timeout: {err}"))?;

    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}:{port}\r\nUser-Agent: Kivarro/0.1\r\nAccept: application/json\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body_bytes.len()
    );
    std::io::Write::write_all(&mut stream, request.as_bytes())
        .map_err(|err| format!("failed to write HTTP request: {err}"))?;
    if !body_bytes.is_empty() {
        std::io::Write::write_all(&mut stream, &body_bytes)
            .map_err(|err| format!("failed to write HTTP request body: {err}"))?;
    }

    let mut raw = Vec::new();
    stream
        .by_ref()
        .take(MAX_HTTP_RESPONSE_BYTES + 1)
        .read_to_end(&mut raw)
        .map_err(|err| format!("failed to read HTTP response: {err}"))?;
    if raw.len() as u64 > MAX_HTTP_RESPONSE_BYTES {
        return Err(format!(
            "llama-server response exceeds {} bytes",
            MAX_HTTP_RESPONSE_BYTES
        ));
    }

    parse_http_response(&raw)
}

fn parse_http_response(raw: &[u8]) -> Result<HttpResponse, String> {
    let header_end = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| "HTTP response did not contain a header terminator".to_string())?;
    let header_text = String::from_utf8_lossy(&raw[..header_end]);
    let mut lines = header_text.lines();
    let status_line = lines
        .next()
        .ok_or_else(|| "HTTP response did not contain a status line".to_string())?;
    let status_code = status_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| format!("invalid HTTP status line: {status_line}"))?
        .parse::<u16>()
        .map_err(|err| format!("invalid HTTP status code: {err}"))?;
    let mut headers = HashMap::new();
    for line in lines {
        if let Some((key, value)) = line.split_once(':') {
            headers.insert(
                key.trim().to_ascii_lowercase(),
                value.trim().to_ascii_lowercase(),
            );
        }
    }

    let mut body = raw[header_end + 4..].to_vec();
    if headers
        .get("transfer-encoding")
        .map(|value| value.contains("chunked"))
        .unwrap_or(false)
    {
        body = decode_chunked_body(&body)?;
    }

    Ok(HttpResponse { status_code, body })
}

fn decode_chunked_body(raw: &[u8]) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    let mut cursor = 0;
    loop {
        let Some(line_end) = raw[cursor..]
            .windows(2)
            .position(|window| window == b"\r\n")
            .map(|position| cursor + position)
        else {
            return Err("chunked response ended before chunk size".to_string());
        };
        let size_line = String::from_utf8_lossy(&raw[cursor..line_end]);
        let size_hex = size_line.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_hex, 16)
            .map_err(|err| format!("invalid chunk size {size_hex}: {err}"))?;
        cursor = line_end + 2;

        if size == 0 {
            return Ok(body);
        }
        if cursor + size + 2 > raw.len() {
            return Err("chunked response ended inside chunk data".to_string());
        }

        body.extend_from_slice(&raw[cursor..cursor + size]);
        cursor += size + 2;
    }
}

fn parse_llama_json_response(response: HttpResponse) -> Result<Value, String> {
    let body = serde_json::from_slice::<Value>(&response.body)
        .map_err(|err| format!("llama-server returned invalid JSON: {err}"))?;
    if response.status_code >= 400 {
        let message = body
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or("llama-server request failed");
        return Err(format!(
            "llama-server HTTP {}: {message}",
            response.status_code
        ));
    }

    Ok(body)
}

fn json_u32(value: &Value, pointer: &str) -> Option<u32> {
    value
        .pointer(pointer)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
}

fn read_model_gguf_metadata(path: &Path) -> Option<GgufMetadataSummary> {
    if !is_gguf_file(path) {
        return None;
    }

    read_gguf_metadata_summary(path).ok().flatten()
}

fn read_gguf_metadata_summary(path: &Path) -> Result<Option<GgufMetadataSummary>, String> {
    let mut reader = GgufReader::open(path)?;
    let magic = reader.read_bytes::<4>()?;
    if &magic != GGUF_MAGIC {
        return Ok(None);
    }

    let version = reader.read_u32()?;
    let tensor_count = reader.read_u64()?;
    let metadata_kv_count = reader.read_u64()?;
    if metadata_kv_count > MAX_GGUF_METADATA_PAIRS {
        return Err(format!(
            "GGUF metadata count {} exceeds safety limit {}",
            metadata_kv_count, MAX_GGUF_METADATA_PAIRS
        ));
    }

    let mut values = HashMap::new();
    for _ in 0..metadata_kv_count {
        let key = reader.read_string(MAX_GGUF_KEY_BYTES, "metadata key")?;
        let value_type = reader.read_u32()?;
        let value = read_gguf_scalar_value(&mut reader, value_type)?;

        if let Some(value) = value {
            if should_retain_gguf_key(&key) {
                values.insert(key, value);
            }
        }
    }

    Ok(Some(build_gguf_metadata_summary(
        version,
        tensor_count,
        values,
    )))
}

fn should_retain_gguf_key(key: &str) -> bool {
    key.starts_with("general.")
        || key.ends_with(".context_length")
        || key.ends_with(".block_count")
        || key.ends_with(".embedding_length")
}

fn build_gguf_metadata_summary(
    version: u32,
    tensor_count: u64,
    values: HashMap<String, GgufScalarValue>,
) -> GgufMetadataSummary {
    let architecture = gguf_string(&values, "general.architecture");
    let context_length = architecture
        .as_deref()
        .and_then(|architecture| gguf_u32(&values, &format!("{architecture}.context_length")))
        .or_else(|| first_gguf_u32_by_suffix(&values, ".context_length"));
    let block_count = architecture
        .as_deref()
        .and_then(|architecture| gguf_u16(&values, &format!("{architecture}.block_count")))
        .or_else(|| first_gguf_u16_by_suffix(&values, ".block_count"));
    let quantization = gguf_u64(&values, "general.file_type").map(gguf_file_type_label);

    GgufMetadataSummary {
        version,
        tensor_count,
        name: gguf_string(&values, "general.name"),
        basename: gguf_string(&values, "general.basename"),
        architecture,
        parameter_size: gguf_string(&values, "general.size_label"),
        quantization,
        context_length,
        block_count,
    }
}

struct GgufReader {
    file: File,
}

impl GgufReader {
    fn open(path: &Path) -> Result<Self, String> {
        File::open(path)
            .map(|file| Self { file })
            .map_err(|err| format!("failed to open GGUF metadata {}: {err}", path.display()))
    }

    fn read_bytes<const N: usize>(&mut self) -> Result<[u8; N], String> {
        let mut bytes = [0; N];
        self.file
            .read_exact(&mut bytes)
            .map_err(|err| format!("failed to read GGUF metadata: {err}"))?;
        Ok(bytes)
    }

    fn read_u8(&mut self) -> Result<u8, String> {
        Ok(self.read_bytes::<1>()?[0])
    }

    fn read_i8(&mut self) -> Result<i8, String> {
        Ok(i8::from_le_bytes(self.read_bytes::<1>()?))
    }

    fn read_u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.read_bytes::<2>()?))
    }

    fn read_i16(&mut self) -> Result<i16, String> {
        Ok(i16::from_le_bytes(self.read_bytes::<2>()?))
    }

    fn read_u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.read_bytes::<4>()?))
    }

    fn read_i32(&mut self) -> Result<i32, String> {
        Ok(i32::from_le_bytes(self.read_bytes::<4>()?))
    }

    fn read_f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_le_bytes(self.read_bytes::<4>()?))
    }

    fn read_u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.read_bytes::<8>()?))
    }

    fn read_i64(&mut self) -> Result<i64, String> {
        Ok(i64::from_le_bytes(self.read_bytes::<8>()?))
    }

    fn read_f64(&mut self) -> Result<f64, String> {
        Ok(f64::from_le_bytes(self.read_bytes::<8>()?))
    }

    fn read_string(&mut self, limit: u64, label: &str) -> Result<String, String> {
        let byte_count = self.read_u64()?;
        if byte_count > limit {
            return Err(format!(
                "GGUF {label} length {} exceeds safety limit {}",
                byte_count, limit
            ));
        }

        let byte_count = usize::try_from(byte_count)
            .map_err(|_| format!("GGUF {label} length cannot fit in memory"))?;
        let mut bytes = vec![0; byte_count];
        self.file
            .read_exact(&mut bytes)
            .map_err(|err| format!("failed to read GGUF {label}: {err}"))?;
        String::from_utf8(bytes).map_err(|err| format!("GGUF {label} is not valid UTF-8: {err}"))
    }

    fn skip_bytes(&mut self, byte_count: u64) -> Result<(), String> {
        let offset = i64::try_from(byte_count)
            .map_err(|_| format!("GGUF skip length {byte_count} exceeds seek range"))?;
        self.file
            .seek(SeekFrom::Current(offset))
            .map(|_| ())
            .map_err(|err| format!("failed to skip GGUF metadata bytes: {err}"))
    }
}

fn read_gguf_scalar_value(
    reader: &mut GgufReader,
    value_type: u32,
) -> Result<Option<GgufScalarValue>, String> {
    match value_type {
        0 => Ok(Some(GgufScalarValue::Unsigned(reader.read_u8()? as u64))),
        1 => Ok(Some(GgufScalarValue::Signed(reader.read_i8()? as i64))),
        2 => Ok(Some(GgufScalarValue::Unsigned(reader.read_u16()? as u64))),
        3 => Ok(Some(GgufScalarValue::Signed(reader.read_i16()? as i64))),
        4 => Ok(Some(GgufScalarValue::Unsigned(reader.read_u32()? as u64))),
        5 => Ok(Some(GgufScalarValue::Signed(reader.read_i32()? as i64))),
        6 => {
            reader.read_f32()?;
            Ok(None)
        }
        7 => {
            reader.read_u8()?;
            Ok(None)
        }
        GGUF_VALUE_TYPE_STRING => Ok(Some(GgufScalarValue::String(
            reader.read_string(MAX_GGUF_STRING_BYTES, "string value")?,
        ))),
        GGUF_VALUE_TYPE_ARRAY => {
            skip_gguf_array(reader, 0)?;
            Ok(None)
        }
        10 => Ok(Some(GgufScalarValue::Unsigned(reader.read_u64()?))),
        11 => Ok(Some(GgufScalarValue::Signed(reader.read_i64()?))),
        12 => {
            reader.read_f64()?;
            Ok(None)
        }
        _ => Err(format!("unsupported GGUF metadata value type {value_type}")),
    }
}

fn skip_gguf_array(reader: &mut GgufReader, depth: u8) -> Result<(), String> {
    if depth >= MAX_GGUF_ARRAY_DEPTH {
        return Err("GGUF metadata array nesting exceeds safety limit".to_string());
    }

    let child_type = reader.read_u32()?;
    let item_count = reader.read_u64()?;
    if item_count > MAX_GGUF_ARRAY_ITEMS {
        return Err(format!(
            "GGUF metadata array length {} exceeds safety limit {}",
            item_count, MAX_GGUF_ARRAY_ITEMS
        ));
    }

    if let Some(item_size) = gguf_fixed_value_size(child_type) {
        let byte_count = item_size
            .checked_mul(item_count)
            .ok_or_else(|| "GGUF metadata array byte count overflowed".to_string())?;
        return reader.skip_bytes(byte_count);
    }

    for _ in 0..item_count {
        skip_gguf_value(reader, child_type, depth + 1)?;
    }

    Ok(())
}

fn skip_gguf_value(reader: &mut GgufReader, value_type: u32, depth: u8) -> Result<(), String> {
    if let Some(item_size) = gguf_fixed_value_size(value_type) {
        return reader.skip_bytes(item_size);
    }

    match value_type {
        GGUF_VALUE_TYPE_STRING => {
            let byte_count = reader.read_u64()?;
            if byte_count > MAX_GGUF_STRING_BYTES {
                return Err(format!(
                    "GGUF string array item length {} exceeds safety limit {}",
                    byte_count, MAX_GGUF_STRING_BYTES
                ));
            }
            reader.skip_bytes(byte_count)
        }
        GGUF_VALUE_TYPE_ARRAY => skip_gguf_array(reader, depth),
        _ => Err(format!("unsupported GGUF metadata value type {value_type}")),
    }
}

fn gguf_fixed_value_size(value_type: u32) -> Option<u64> {
    match value_type {
        0 | 1 | 7 => Some(1),
        2 | 3 => Some(2),
        4 | 5 | 6 => Some(4),
        10 | 11 | 12 => Some(8),
        _ => None,
    }
}

fn gguf_string(values: &HashMap<String, GgufScalarValue>, key: &str) -> Option<String> {
    match values.get(key) {
        Some(GgufScalarValue::String(value)) if !value.trim().is_empty() => {
            Some(value.trim().to_string())
        }
        _ => None,
    }
}

fn gguf_u64(values: &HashMap<String, GgufScalarValue>, key: &str) -> Option<u64> {
    match values.get(key) {
        Some(GgufScalarValue::Unsigned(value)) => Some(*value),
        Some(GgufScalarValue::Signed(value)) if *value >= 0 => Some(*value as u64),
        _ => None,
    }
}

fn gguf_u32(values: &HashMap<String, GgufScalarValue>, key: &str) -> Option<u32> {
    gguf_u64(values, key).and_then(|value| u32::try_from(value).ok())
}

fn gguf_u16(values: &HashMap<String, GgufScalarValue>, key: &str) -> Option<u16> {
    gguf_u64(values, key).and_then(|value| u16::try_from(value).ok())
}

fn first_gguf_u32_by_suffix(
    values: &HashMap<String, GgufScalarValue>,
    suffix: &str,
) -> Option<u32> {
    values
        .iter()
        .find(|(key, _)| key.ends_with(suffix))
        .and_then(|(key, _)| gguf_u32(values, key))
}

fn first_gguf_u16_by_suffix(
    values: &HashMap<String, GgufScalarValue>,
    suffix: &str,
) -> Option<u16> {
    values
        .iter()
        .find(|(key, _)| key.ends_with(suffix))
        .and_then(|(key, _)| gguf_u16(values, key))
}

fn gguf_file_type_label(file_type: u64) -> String {
    match file_type {
        0 => "F32",
        1 => "F16",
        2 => "Q4_0",
        3 => "Q4_1",
        4 => "Q4_1+F16",
        5 => "Q4_2",
        6 => "Q4_3",
        7 => "Q8_0",
        8 => "Q5_0",
        9 => "Q5_1",
        10 => "Q2_K",
        11 => "Q3_K_S",
        12 => "Q3_K_M",
        13 => "Q3_K_L",
        14 => "Q4_K_S",
        15 => "Q4_K_M",
        16 => "Q5_K_S",
        17 => "Q5_K_M",
        18 => "Q6_K",
        _ => return format!("FILE_TYPE_{file_type}"),
    }
    .to_string()
}

fn gguf_display_name(summary: &GgufMetadataSummary, fallback: &str) -> String {
    if let Some(name) = summary.name.as_deref().filter(|name| !name.is_empty()) {
        return name.to_string();
    }

    match (
        summary.basename.as_deref(),
        summary.parameter_size.as_deref(),
    ) {
        (Some(basename), Some(size)) if !basename.is_empty() && !size.is_empty() => {
            format!("{basename} {size}")
        }
        (Some(basename), _) if !basename.is_empty() => basename.to_string(),
        _ => fallback.to_string(),
    }
}

fn infer_quantization_from_name(model_name: &str) -> Option<String> {
    let upper = model_name.to_ascii_uppercase();
    [
        "Q8_0", "Q6_K", "Q5_K_M", "Q5_K_S", "Q5_1", "Q5_0", "Q4_K_M", "Q4_K_S", "Q4_1", "Q4_0",
        "Q3_K_L", "Q3_K_M", "Q3_K_S", "Q2_K", "F16", "F32",
    ]
    .iter()
    .find(|quantization| upper.contains(**quantization))
    .map(|quantization| (*quantization).to_string())
}

fn estimate_layer_count(model_name: &str, block_count: Option<u16>) -> u16 {
    if let Some(block_count) = block_count.filter(|count| *count > 0) {
        return block_count;
    }

    let lower = model_name.to_ascii_lowercase();
    if lower.contains("405b") {
        126
    } else if lower.contains("120b") {
        96
    } else if lower.contains("70b") || lower.contains("72b") {
        80
    } else if lower.contains("34b") || lower.contains("32b") {
        64
    } else if lower.contains("14b") || lower.contains("13b") {
        40
    } else if lower.contains("8b") || lower.contains("7b") {
        32
    } else if lower.contains("3b") {
        28
    } else {
        32
    }
}

fn estimate_kv_cache_gib(context_length: u32, estimated_layers: u16, quantization: &str) -> f64 {
    let precision_factor = match quantization.to_ascii_lowercase().as_str() {
        "q4_0" | "q4_1" | "q5_0" | "q5_1" => 0.35,
        "q8_0" | "q8_1" => 0.55,
        "f32" => 2.0,
        _ => 1.0,
    };
    let baseline_per_token_layer_gib = 0.0000048;
    round_gib(
        context_length as f64
            * estimated_layers as f64
            * baseline_per_token_layer_gib
            * precision_factor,
    )
}

fn model_search_paths() -> Result<Vec<PathBuf>, String> {
    let current_dir =
        env::current_dir().map_err(|err| format!("failed to read current directory: {err}"))?;
    Ok(vec![current_dir.join("models")])
}

fn collect_models(directory: &Path, models: &mut Vec<ModelRecord>) -> Result<(), String> {
    if !directory.exists() {
        return Ok(());
    }

    let entries = fs::read_dir(directory).map_err(|err| {
        format!(
            "failed to read model directory {}: {err}",
            directory.display()
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(|err| format!("failed to read model directory entry: {err}"))?;
        let path = entry.path();

        if path.is_dir() {
            collect_models(&path, models)?;
            continue;
        }

        if !is_supported_model_file(&path) {
            continue;
        }

        let metadata = entry
            .metadata()
            .map_err(|err| format!("failed to read model metadata {}: {err}", path.display()))?;
        let fallback_name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("Unnamed model")
            .to_string();
        let gguf_metadata = read_model_gguf_metadata(&path);
        let name = gguf_metadata
            .as_ref()
            .map(|summary| gguf_display_name(summary, &fallback_name))
            .unwrap_or_else(|| fallback_name.clone());
        let format = path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("unknown")
            .to_ascii_uppercase();
        let size_gib = bytes_to_gib(metadata.len());
        let architecture = gguf_metadata
            .as_ref()
            .and_then(|summary| summary.architecture.clone());
        let parameter_size = gguf_metadata
            .as_ref()
            .and_then(|summary| summary.parameter_size.clone());
        let quantization = gguf_metadata
            .as_ref()
            .and_then(|summary| summary.quantization.clone())
            .or_else(|| infer_quantization_from_name(&fallback_name));
        let context_length = gguf_metadata
            .as_ref()
            .and_then(|summary| summary.context_length);
        let block_count = gguf_metadata
            .as_ref()
            .and_then(|summary| summary.block_count);
        let tensor_count = gguf_metadata.as_ref().map(|summary| summary.tensor_count);
        let gguf_version = gguf_metadata.as_ref().map(|summary| summary.version);
        let metadata_source = gguf_metadata
            .as_ref()
            .map(|summary| format!("GGUF v{}", summary.version))
            .unwrap_or_else(|| "filename".to_string());
        let status = if gguf_metadata.is_some() {
            "Indexed"
        } else {
            "Discovered"
        };

        models.push(ModelRecord {
            id: path.to_string_lossy().to_string(),
            name,
            path: path.to_string_lossy().to_string(),
            format,
            size_gib,
            status: status.to_string(),
            fit: model_fit_label(size_gib),
            architecture,
            parameter_size,
            quantization,
            context_length,
            block_count,
            tensor_count,
            gguf_version,
            metadata_source,
        });
    }

    Ok(())
}

fn is_gguf_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.eq_ignore_ascii_case("gguf"))
        .unwrap_or(false)
}

fn is_supported_model_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "gguf" | "safetensors" | "bin" | "mlx"
            )
        })
        .unwrap_or(false)
}

fn model_fit_label(size_gib: f64) -> String {
    let mut system = System::new_all();
    system.refresh_memory();
    let ram_total_gib = bytes_to_gib(system.total_memory());

    if size_gib * 1.35 <= ram_total_gib {
        "Fits".to_string()
    } else if size_gib <= ram_total_gib {
        "May be slow".to_string()
    } else {
        "Won't fit".to_string()
    }
}

fn bytes_to_gib(bytes: u64) -> f64 {
    let gib = bytes as f64 / 1024.0 / 1024.0 / 1024.0;
    round_gib(gib)
}

fn round_gib(gib: f64) -> f64 {
    (gib * 10.0).round() / 10.0
}

fn percent(used: f64, total: f64) -> f32 {
    if total <= 0.0 {
        return 0.0;
    }

    ((used / total) * 100.0).clamp(0.0, 100.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parses_minimal_gguf_metadata() {
        let path = env::temp_dir().join(format!(
            "kivarro-gguf-metadata-{}-{}.gguf",
            std::process::id(),
            unix_timestamp()
        ));
        let mut bytes = Vec::new();
        bytes.extend_from_slice(GGUF_MAGIC);
        bytes.extend_from_slice(&3_u32.to_le_bytes());
        bytes.extend_from_slice(&123_u64.to_le_bytes());
        bytes.extend_from_slice(&6_u64.to_le_bytes());

        push_gguf_string_kv(&mut bytes, "general.architecture", "llama");
        push_gguf_string_kv(&mut bytes, "general.name", "Kivarro Test Model");
        push_gguf_string_kv(&mut bytes, "general.size_label", "7B");
        push_gguf_u32_kv(&mut bytes, "general.file_type", 15);
        push_gguf_u64_kv(&mut bytes, "llama.context_length", 32_768);
        push_gguf_u64_kv(&mut bytes, "llama.block_count", 32);

        let mut file = File::create(&path).expect("create synthetic GGUF");
        file.write_all(&bytes).expect("write synthetic GGUF");
        drop(file);

        let summary = read_gguf_metadata_summary(&path)
            .expect("parse synthetic GGUF")
            .expect("GGUF metadata summary");

        assert_eq!(summary.version, 3);
        assert_eq!(summary.tensor_count, 123);
        assert_eq!(summary.name.as_deref(), Some("Kivarro Test Model"));
        assert_eq!(summary.architecture.as_deref(), Some("llama"));
        assert_eq!(summary.parameter_size.as_deref(), Some("7B"));
        assert_eq!(summary.quantization.as_deref(), Some("Q4_K_M"));
        assert_eq!(summary.context_length, Some(32_768));
        assert_eq!(summary.block_count, Some(32));

        fs::remove_file(path).expect("remove synthetic GGUF");
    }

    #[test]
    fn builds_llama_server_args_from_profile_runtime() {
        let mut profile = default_profiles()
            .into_iter()
            .next()
            .expect("default profile");
        profile.runtime.context_length = 65_536;
        profile.runtime.cpu_threads = 12;
        profile.runtime.batch_size = 1024;
        profile.runtime.micro_batch_size = 256;
        profile.runtime.gpu_layers = 41;
        profile.runtime.kv_cache_quantization = "q8_0".to_string();
        profile.runtime.use_mmap = false;
        profile.runtime.use_mlock = true;
        profile.runtime.flash_attention = false;
        profile.runtime.main_gpu = Some(1);
        profile.runtime.tensor_split = vec![3.0, 1.0];
        profile.runtime.rope_frequency_base = Some(1_000_000.0);
        profile.runtime.rope_frequency_scale = Some(0.5);

        let args = build_llama_server_args(
            Path::new("D:/models/test.gguf"),
            &profile,
            DEFAULT_API_HOST,
            9090,
        );

        assert_arg_pair(&args, "--model", "D:/models/test.gguf");
        assert_arg_pair(&args, "--host", DEFAULT_API_HOST);
        assert_arg_pair(&args, "--port", "9090");
        assert_arg_pair(&args, "--ctx-size", "65536");
        assert_arg_pair(&args, "--threads", "12");
        assert_arg_pair(&args, "--batch-size", "1024");
        assert_arg_pair(&args, "--ubatch-size", "256");
        assert_arg_pair(&args, "--gpu-layers", "41");
        assert_arg_pair(&args, "--cache-type-k", "q8_0");
        assert_arg_pair(&args, "--cache-type-v", "q8_0");
        assert_arg_pair(&args, "--flash-attn", "off");
        assert_arg_pair(&args, "--main-gpu", "1");
        assert_arg_pair(&args, "--tensor-split", "3,1");
        assert_arg_pair(&args, "--rope-freq-base", "1000000");
        assert_arg_pair(&args, "--rope-freq-scale", "0.5");
        assert!(args.iter().any(|arg| arg == "--no-mmap"));
        assert!(args.iter().any(|arg| arg == "--mlock"));
    }

    fn push_gguf_string_kv(bytes: &mut Vec<u8>, key: &str, value: &str) {
        push_gguf_string(bytes, key);
        bytes.extend_from_slice(&GGUF_VALUE_TYPE_STRING.to_le_bytes());
        push_gguf_string(bytes, value);
    }

    fn push_gguf_u32_kv(bytes: &mut Vec<u8>, key: &str, value: u32) {
        push_gguf_string(bytes, key);
        bytes.extend_from_slice(&4_u32.to_le_bytes());
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn push_gguf_u64_kv(bytes: &mut Vec<u8>, key: &str, value: u64) {
        push_gguf_string(bytes, key);
        bytes.extend_from_slice(&10_u32.to_le_bytes());
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn push_gguf_string(bytes: &mut Vec<u8>, value: &str) {
        bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
        bytes.extend_from_slice(value.as_bytes());
    }

    fn assert_arg_pair(args: &[String], key: &str, value: &str) {
        let position = args
            .iter()
            .position(|arg| arg == key)
            .unwrap_or_else(|| panic!("{key} missing from args: {args:?}"));
        assert_eq!(args.get(position + 1).map(String::as_str), Some(value));
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(EngineRuntime::default())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_hardware_snapshot,
            get_runtime_metrics,
            list_models,
            list_inference_profiles,
            save_inference_profile,
            delete_inference_profile,
            get_model_load_plan,
            get_engine_status,
            start_llama_server,
            stop_llama_server,
            run_chat_completion,
            get_api_status,
            list_benchmark_results,
            list_system_logs
        ])
        .run(tauri::generate_context!())
        .expect("error while running Kivarro");
}
