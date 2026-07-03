use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::{
    collections::HashMap,
    env, fs,
    fs::File,
    io::{ErrorKind, Read, Seek, SeekFrom},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command as ProcessCommand, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use sysinfo::System;
use tauri::{AppHandle, Emitter, Manager, State, Window};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const PROFILE_EXTENSION: &str = "kivarro.json";
const API_SETTINGS_FILE: &str = "api-settings.json";
const BENCHMARK_RESULTS_FILE: &str = "benchmarks.json";
const MAX_BENCHMARK_RESULTS: usize = 200;
const SYSTEM_LOG_FILE: &str = "system-logs.json";
const MAX_SYSTEM_LOGS: usize = 1_000;
const KNOWLEDGE_STORE_FILE: &str = "knowledge-store.json";
const MAX_KNOWLEDGE_DOCUMENT_BYTES: u64 = 8 * 1024 * 1024;
const KNOWLEDGE_CHUNK_TARGET_CHARS: usize = 1_200;
const KNOWLEDGE_CHUNK_OVERLAP_CHARS: usize = 160;
const MAX_RETRIEVAL_RESULTS: usize = 8;
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
const BACKEND_LLAMA_CPP: &str = "llama.cpp";
const BACKEND_MISTRAL_RS: &str = "mistral.rs";
const LLAMA_SERVER_ENV: &str = "KIVARRO_LLAMA_SERVER";
const MISTRALRS_ENV: &str = "KIVARRO_MISTRALRS";
const API_PORT_ENV: &str = "KIVARRO_API_PORT";
const MAX_HTTP_RESPONSE_BYTES: u64 = 16 * 1024 * 1024;
const HTTP_HEALTH_TIMEOUT_MS: u64 = 400;
const HTTP_CHAT_TIMEOUT_MS: u64 = 3_600_000;
const MAX_HTTP_HEADER_BYTES: usize = 64 * 1024;
const MAX_STREAM_RESPONSE_BYTES: u64 = 128 * 1024 * 1024;
const STREAM_READ_POLL_MS: u64 = 200;
const STREAM_EVENT_NAME: &str = "kivarro://chat-stream";

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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelImportResult {
    imported: ModelRecord,
    models: Vec<ModelRecord>,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ApiSettings {
    host: String,
    port: u16,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct BenchmarkResult {
    model: String,
    backend: String,
    eval_count: u32,
    eval_duration_ms: u64,
    tokens_per_second: f32,
    load_duration_ms: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct KnowledgeBase {
    id: String,
    name: String,
    document_count: u32,
    chunk_count: u32,
    updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct KnowledgeDocument {
    id: String,
    knowledge_base_id: String,
    name: String,
    path: String,
    size_bytes: u64,
    chunk_count: u32,
    imported_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct KnowledgeChunk {
    id: String,
    knowledge_base_id: String,
    document_id: String,
    document_name: String,
    chunk_index: u32,
    content: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct KnowledgeStore {
    bases: Vec<KnowledgeBase>,
    documents: Vec<KnowledgeDocument>,
    chunks: Vec<KnowledgeChunk>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct KnowledgeBaseDetail {
    base: KnowledgeBase,
    documents: Vec<KnowledgeDocument>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RetrievalMatch {
    knowledge_base_id: String,
    document_id: String,
    document_name: String,
    chunk_index: u32,
    score: f32,
    snippet: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct InferenceStreamEvent {
    request_id: String,
    phase: String,
    delta: String,
    content: String,
    model: String,
    completion_tokens: u32,
    tokens_per_second: f32,
    elapsed_ms: u128,
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

#[derive(Debug, Default)]
struct StreamChunk {
    delta: String,
    finish_reason: Option<String>,
    stop: bool,
}

#[derive(Debug, Default)]
struct StreamAccumulator {
    content: String,
    completion_tokens: u32,
    finish_reason: Option<String>,
}

struct ActiveEngineContext {
    host: String,
    port: u16,
    backend: String,
    active_model_id: String,
    active_model_name: String,
    request_model_name: String,
}

struct EngineRuntime {
    inner: Mutex<ManagedEngine>,
    stream_cancellations: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

struct ManagedEngine {
    child: Option<Child>,
    active_backend: String,
    active_model_id: Option<String>,
    active_model_name: Option<String>,
    active_request_model: Option<String>,
    load_started_at: Option<Instant>,
    last_load_duration_ms: u64,
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
            stream_cancellations: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for ManagedEngine {
    fn default() -> Self {
        Self {
            child: None,
            active_backend: BACKEND_LLAMA_CPP.to_string(),
            active_model_id: None,
            active_model_name: None,
            active_request_model: None,
            load_started_at: None,
            last_load_duration_ms: 0,
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

    let mut blocks = vec![
        ComputeBlock {
            id: "cpu".to_string(),
            name: cpu_brand.clone(),
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
    ];
    let gpu_blocks = detect_gpu_blocks();
    if gpu_blocks.is_empty() {
        blocks.push(ComputeBlock {
            id: "gpu-probe".to_string(),
            name: "Accelerator Probe".to_string(),
            kind: "GPU".to_string(),
            status: "No GPU telemetry source detected".to_string(),
            utilization_percent: 0.0,
            memory_total_gib: None,
            memory_used_gib: None,
            segments: Vec::new(),
        });
    } else {
        blocks.extend(gpu_blocks);
    }

    Ok(HardwareSnapshot {
        os: env::consts::OS.to_string(),
        architecture: env::consts::ARCH.to_string(),
        cpu_brand: cpu_brand.clone(),
        cpu_cores: system.cpus().len(),
        cpu_utilization_percent,
        ram_total_gib,
        ram_used_gib,
        blocks,
    })
}

fn detect_gpu_blocks() -> Vec<ComputeBlock> {
    let nvidia_blocks = detect_nvidia_smi_gpus();
    if !nvidia_blocks.is_empty() {
        return nvidia_blocks;
    }

    detect_platform_gpu_blocks()
}

fn detect_nvidia_smi_gpus() -> Vec<ComputeBlock> {
    let Some(binary) = find_executable_on_path("nvidia-smi") else {
        return Vec::new();
    };
    let Some(output) = command_output_text(
        &binary,
        &[
            "--query-gpu=name,utilization.gpu,memory.total,memory.used",
            "--format=csv,noheader,nounits",
        ],
    ) else {
        return Vec::new();
    };

    parse_nvidia_smi_gpus(&output)
}

fn parse_nvidia_smi_gpus(output: &str) -> Vec<ComputeBlock> {
    output
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let parts = line.split(',').map(str::trim).collect::<Vec<_>>();
            if parts.len() < 4 || parts[0].is_empty() {
                return None;
            }

            let utilization_percent = parts[1].trim_end_matches('%').trim().parse().unwrap_or(0.0);
            let memory_total_gib = mib_to_gib(parts[2].parse::<f64>().unwrap_or(0.0));
            let memory_used_gib = mib_to_gib(parts[3].parse::<f64>().unwrap_or(0.0));

            Some(ComputeBlock {
                id: format!("gpu-nvidia-{index}"),
                name: parts[0].to_string(),
                kind: "GPU".to_string(),
                status: "NVIDIA telemetry active".to_string(),
                utilization_percent,
                memory_total_gib: Some(memory_total_gib),
                memory_used_gib: Some(memory_used_gib),
                segments: gpu_memory_segments(memory_total_gib, memory_used_gib),
            })
        })
        .collect()
}

fn gpu_memory_segments(memory_total_gib: f64, memory_used_gib: f64) -> Vec<MemorySegment> {
    vec![
        MemorySegment {
            label: "Used VRAM".to_string(),
            gib: memory_used_gib,
            color: "red".to_string(),
        },
        MemorySegment {
            label: "Free VRAM".to_string(),
            gib: (memory_total_gib - memory_used_gib).max(0.0),
            color: "cyan".to_string(),
        },
    ]
}

fn mib_to_gib(mib: f64) -> f64 {
    round_gib(mib / 1024.0)
}

fn command_output_text(program: &Path, args: &[&str]) -> Option<String> {
    let mut command = ProcessCommand::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(target_os = "windows")]
    command.creation_flags(WINDOWS_CREATE_NO_WINDOW);

    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout).ok()
}

#[cfg(target_os = "windows")]
fn detect_platform_gpu_blocks() -> Vec<ComputeBlock> {
    let Some(binary) =
        find_executable_on_path("powershell").or_else(|| find_executable_on_path("pwsh"))
    else {
        return Vec::new();
    };
    let Some(output) = command_output_text(
        &binary,
        &[
            "-NoProfile",
            "-Command",
            "Get-CimInstance Win32_VideoController | Select-Object Name,AdapterRAM | ConvertTo-Csv -NoTypeInformation",
        ],
    ) else {
        return Vec::new();
    };

    parse_windows_video_controllers(&output)
}

#[cfg(target_os = "windows")]
fn parse_windows_video_controllers(output: &str) -> Vec<ComputeBlock> {
    output
        .lines()
        .skip(1)
        .enumerate()
        .filter_map(|(index, line)| {
            let fields = parse_csv_line(line);
            let name = fields.first()?.trim().to_string();
            if name.is_empty() {
                return None;
            }
            let memory_total_gib = fields
                .get(1)
                .and_then(|value| value.trim().parse::<u64>().ok())
                .filter(|bytes| *bytes > 0)
                .map(bytes_to_gib);

            Some(ComputeBlock {
                id: format!("gpu-windows-{index}"),
                name,
                kind: "GPU".to_string(),
                status: if memory_total_gib.is_some() {
                    "Discovered via WMI; live usage unavailable".to_string()
                } else {
                    "Discovered via WMI".to_string()
                },
                utilization_percent: 0.0,
                memory_total_gib,
                memory_used_gib: None,
                segments: Vec::new(),
            })
        })
        .collect()
}

#[cfg(target_os = "macos")]
fn detect_platform_gpu_blocks() -> Vec<ComputeBlock> {
    let Some(binary) = find_executable_on_path("system_profiler") else {
        return Vec::new();
    };
    let Some(output) = command_output_text(&binary, &["SPDisplaysDataType"]) else {
        return Vec::new();
    };

    parse_macos_displays(&output)
}

#[cfg(target_os = "macos")]
fn parse_macos_displays(output: &str) -> Vec<ComputeBlock> {
    let mut blocks = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_vram: Option<f64> = None;

    for line in output.lines() {
        let trimmed = line.trim();
        if let Some(name) = trimmed.strip_prefix("Chipset Model:") {
            if let Some(previous_name) = current_name.take() {
                push_macos_gpu_block(&mut blocks, previous_name, current_vram.take());
            }
            current_name = Some(name.trim().to_string());
        } else if trimmed.starts_with("VRAM") {
            current_vram = parse_vram_gib(trimmed);
        }
    }

    if let Some(previous_name) = current_name {
        push_macos_gpu_block(&mut blocks, previous_name, current_vram);
    }

    blocks
}

#[cfg(target_os = "macos")]
fn push_macos_gpu_block(
    blocks: &mut Vec<ComputeBlock>,
    name: String,
    memory_total_gib: Option<f64>,
) {
    blocks.push(ComputeBlock {
        id: format!("gpu-macos-{}", blocks.len()),
        name,
        kind: "GPU".to_string(),
        status: "Discovered via system_profiler; live usage unavailable".to_string(),
        utilization_percent: 0.0,
        memory_total_gib,
        memory_used_gib: None,
        segments: Vec::new(),
    });
}

#[cfg(target_os = "macos")]
fn parse_vram_gib(line: &str) -> Option<f64> {
    let (_, value) = line.split_once(':')?;
    let mut parts = value.split_whitespace();
    let amount = parts.next()?.parse::<f64>().ok()?;
    let unit = parts.next().unwrap_or("MB").to_ascii_lowercase();
    if unit.starts_with("gb") {
        Some(round_gib(amount))
    } else if unit.starts_with("mb") {
        Some(mib_to_gib(amount))
    } else {
        None
    }
}

#[cfg(target_os = "linux")]
fn detect_platform_gpu_blocks() -> Vec<ComputeBlock> {
    let Some(binary) = find_executable_on_path("lspci") else {
        return Vec::new();
    };
    let Some(output) = command_output_text(&binary, &["-mm"]) else {
        return Vec::new();
    };

    parse_linux_lspci_gpus(&output)
}

#[cfg(target_os = "linux")]
fn parse_linux_lspci_gpus(output: &str) -> Vec<ComputeBlock> {
    output
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let fields = parse_quoted_fields(line);
            if fields.len() < 4 {
                return None;
            }
            let class = fields.get(1)?;
            if !["VGA", "3D controller", "Display controller"]
                .iter()
                .any(|needle| class.contains(needle))
            {
                return None;
            }
            let vendor = fields.get(2).map(String::as_str).unwrap_or("");
            let device = fields.get(3).map(String::as_str).unwrap_or("");
            let name = format!("{vendor} {device}").trim().to_string();
            if name.is_empty() {
                return None;
            }

            Some(ComputeBlock {
                id: format!("gpu-linux-{index}"),
                name,
                kind: "GPU".to_string(),
                status: "Discovered via lspci; VRAM usage unavailable".to_string(),
                utilization_percent: 0.0,
                memory_total_gib: None,
                memory_used_gib: None,
                segments: Vec::new(),
            })
        })
        .collect()
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn detect_platform_gpu_blocks() -> Vec<ComputeBlock> {
    Vec::new()
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut chars = line.chars().peekable();
    let mut in_quotes = false;

    while let Some(ch) = chars.next() {
        match ch {
            '"' if in_quotes && matches!(chars.peek(), Some('"')) => {
                field.push('"');
                chars.next();
            }
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                fields.push(field.trim().to_string());
                field.clear();
            }
            _ => field.push(ch),
        }
    }
    fields.push(field.trim().to_string());

    fields
}

#[cfg(target_os = "linux")]
fn parse_quoted_fields(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;

    for ch in line.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ' ' | '\t' if !in_quotes => {
                if !field.trim().is_empty() {
                    fields.push(field.trim().to_string());
                    field.clear();
                }
            }
            _ => field.push(ch),
        }
    }
    if !field.trim().is_empty() {
        fields.push(field.trim().to_string());
    }

    fields
}

#[tauri::command]
fn get_runtime_metrics(
    app_handle: AppHandle,
    engine: State<'_, EngineRuntime>,
) -> Result<RuntimeMetrics, String> {
    let settings = read_api_settings(&app_handle)?;
    sync_engine_endpoint_if_idle(&engine, &settings)?;
    let snapshot = get_hardware_snapshot()?;
    let status = current_engine_status(&engine)?;
    let gpu_utilization_percent = snapshot
        .blocks
        .iter()
        .filter(|block| block.kind == "GPU")
        .map(|block| block.utilization_percent)
        .fold(0.0_f32, f32::max);

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
        gpu_utilization_percent,
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
fn import_model_file(app_handle: AppHandle, path: String) -> Result<ModelImportResult, String> {
    let raw_path = path.trim();
    if raw_path.is_empty() {
        return Err("model file path is required".to_string());
    }

    let source = fs::canonicalize(raw_path)
        .map_err(|err| format!("failed to resolve model file {raw_path}: {err}"))?;
    if !source.is_file() {
        return Err(format!("model path is not a file: {}", source.display()));
    }
    if !is_supported_model_file(&source) {
        return Err("model file must be .gguf, .safetensors, .bin, or .mlx".to_string());
    }

    let destination_dir = model_library_directory()?;
    fs::create_dir_all(&destination_dir).map_err(|err| {
        format!(
            "failed to create model library {}: {err}",
            destination_dir.display()
        )
    })?;

    let library_root =
        fs::canonicalize(&destination_dir).unwrap_or_else(|_| destination_dir.clone());
    let destination = if source.starts_with(&library_root) {
        source.clone()
    } else {
        let file_name = source
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| "model file name is not valid UTF-8".to_string())?;
        let destination = unique_model_destination(&destination_dir, file_name);
        fs::copy(&source, &destination).map_err(|err| {
            format!(
                "failed to copy model {} to {}: {err}",
                source.display(),
                destination.display()
            )
        })?;
        destination
    };

    let imported = model_record_from_path(&destination)?;
    let models = list_models()?;
    let _ = append_system_log(
        &app_handle,
        "INFO",
        "registry",
        format!("Imported model {}", imported.name),
    );

    Ok(ModelImportResult { imported, models })
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

    let _ = append_system_log(
        &app_handle,
        "INFO",
        "profiles",
        format!("Saved inference profile {}", profile.name),
    );

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
fn get_engine_status(
    app_handle: AppHandle,
    engine: State<'_, EngineRuntime>,
) -> Result<EngineStatus, String> {
    let settings = read_api_settings(&app_handle)?;
    sync_engine_endpoint_if_idle(&engine, &settings)?;
    current_engine_status(&engine)
}

#[tauri::command]
fn start_inference_engine(
    app_handle: AppHandle,
    engine: State<'_, EngineRuntime>,
    model_id: String,
    profile: InferenceProfile,
) -> Result<EngineStatus, String> {
    let backend = profile_backend(&profile)?;
    start_engine_for_backend(&app_handle, engine, model_id, profile, backend)
}

#[tauri::command]
fn start_llama_server(
    app_handle: AppHandle,
    engine: State<'_, EngineRuntime>,
    model_id: String,
    profile: InferenceProfile,
) -> Result<EngineStatus, String> {
    start_engine_for_backend(&app_handle, engine, model_id, profile, BACKEND_LLAMA_CPP)
}

fn start_engine_for_backend(
    app_handle: &AppHandle,
    engine: State<'_, EngineRuntime>,
    model_id: String,
    mut profile: InferenceProfile,
    backend: &'static str,
) -> Result<EngineStatus, String> {
    profile.runtime.backend = backend.to_string();
    validate_profile(&profile)?;

    let model_path = canonical_model_path(&model_id)?;
    if backend == BACKEND_LLAMA_CPP && !is_gguf_file(&model_path) {
        return Err("llama.cpp requires a GGUF model file.".to_string());
    }
    let canonical_model_id = model_path.to_string_lossy().to_string();

    let binary_path =
        find_engine_binary(backend).ok_or_else(|| engine_binary_missing_message(backend))?;
    let model_name = model_display_name_from_path(&model_path);
    let request_model = request_model_name(backend, &model_name);
    let api_settings = read_api_settings(app_handle)?;
    let port = api_settings.port;
    let host = api_settings.host;
    let args = build_engine_args(backend, &model_path, &profile, &host, port);

    let mut guard = engine
        .inner
        .lock()
        .map_err(|_| "engine state lock is poisoned".to_string())?;

    refresh_engine_process(&mut guard);
    if guard.child.is_some()
        && guard.active_backend == backend
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
            "failed to start {} at {}: {err}",
            engine_process_label(backend),
            binary_path.display()
        )
    })?;

    guard.child = Some(child);
    guard.active_backend = backend.to_string();
    guard.active_model_id = Some(canonical_model_id);
    guard.active_model_name = Some(model_name.clone());
    guard.active_request_model = Some(request_model);
    guard.load_started_at = Some(Instant::now());
    guard.host = host.clone();
    guard.port = port;
    guard.last_error = None;
    guard.last_tokens_per_second = 0.0;
    guard.last_load_duration_ms = 0;
    guard.context_used_tokens = 0;
    guard.context_total_tokens = profile.runtime.context_length;

    let _ = append_system_log(
        app_handle,
        "INFO",
        "engine",
        format!("Started {backend} for {model_name} on {host}:{port}"),
    );

    Ok(engine_status_from_guard(&mut guard, Some(binary_path)))
}

#[tauri::command]
fn stop_inference_engine(
    app_handle: AppHandle,
    engine: State<'_, EngineRuntime>,
) -> Result<EngineStatus, String> {
    let settings = read_api_settings(&app_handle)?;
    let mut guard = engine
        .inner
        .lock()
        .map_err(|_| "engine state lock is poisoned".to_string())?;

    stop_child(&mut guard)?;
    guard.active_model_id = None;
    guard.active_model_name = None;
    guard.active_request_model = None;
    guard.load_started_at = None;
    guard.last_load_duration_ms = 0;
    guard.last_error = None;
    guard.last_tokens_per_second = 0.0;
    guard.context_used_tokens = 0;
    guard.host = settings.host;
    guard.port = settings.port;
    let backend = guard.active_backend.clone();

    let _ = append_system_log(
        &app_handle,
        "INFO",
        "engine",
        format!("Stopped {backend} engine"),
    );

    Ok(engine_status_from_guard(
        &mut guard,
        find_engine_binary(&backend),
    ))
}

#[tauri::command]
fn stop_llama_server(
    app_handle: AppHandle,
    engine: State<'_, EngineRuntime>,
) -> Result<EngineStatus, String> {
    stop_inference_engine(app_handle, engine)
}

#[tauri::command]
fn cancel_chat_completion_stream(
    engine: State<'_, EngineRuntime>,
    request_id: String,
) -> Result<bool, String> {
    let request_id = request_id.trim();
    if request_id.is_empty() {
        return Err("request id is required".to_string());
    }

    let guard = engine
        .stream_cancellations
        .lock()
        .map_err(|_| "stream cancellation registry lock is poisoned".to_string())?;
    if let Some(cancelled) = guard.get(request_id) {
        cancelled.store(true, Ordering::SeqCst);
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
fn run_chat_completion(
    engine: State<'_, EngineRuntime>,
    model_id: String,
    profile: InferenceProfile,
    prompt: String,
    history: Vec<ChatTurn>,
) -> Result<InferenceRunResult, String> {
    run_chat_completion_inner(&engine, model_id, profile, prompt, history)
}

fn run_chat_completion_inner(
    engine: &State<'_, EngineRuntime>,
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
    let active_context = active_engine_context(&engine, &requested_model_id)?;
    let process_label = engine_process_label(&active_context.backend);

    match probe_engine_health(
        &active_context.host,
        active_context.port,
        HTTP_HEALTH_TIMEOUT_MS,
    ) {
        HealthProbe::Ready => {}
        HealthProbe::Loading(message) => {
            return Err(format!("{process_label} is still loading: {message}"));
        }
        HealthProbe::Offline(message) => {
            return Err(format!("{process_label} is not reachable: {message}"));
        }
    }

    let started = SystemTime::now();
    let payload = build_chat_completion_payload(
        &profile,
        &active_context.request_model_name,
        prompt,
        &history,
    )?;
    let response = http_json_request(
        &active_context.host,
        active_context.port,
        "POST",
        "/v1/chat/completions",
        Some(&payload),
        HTTP_CHAT_TIMEOUT_MS,
    )?;
    let elapsed_ms = started
        .elapsed()
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or_default();
    let body = parse_openai_json_response(response, process_label)?;
    let content = body
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .or_else(|| body.pointer("/choices/0/text").and_then(Value::as_str))
        .unwrap_or_default()
        .to_string();

    if content.is_empty() {
        return Err(format!("{process_label} returned an empty completion"));
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
        guard.active_model_id = Some(active_context.active_model_id);
        guard.active_model_name = Some(active_context.active_model_name.clone());
        guard.last_error = None;
    }

    Ok(InferenceRunResult {
        content,
        model: active_context.active_model_name,
        backend: active_context.backend,
        elapsed_ms,
        tokens_per_second,
        prompt_tokens,
        completion_tokens,
        total_tokens,
        finish_reason,
    })
}

#[tauri::command]
fn run_chat_completion_stream(
    window: Window,
    engine: State<'_, EngineRuntime>,
    request_id: String,
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
    let request_id = if request_id.trim().is_empty() {
        format!("stream-{}", unix_timestamp())
    } else {
        request_id.trim().to_string()
    };
    let requested_model_id = canonical_model_path(&model_id)?
        .to_string_lossy()
        .to_string();
    let cancel_token = register_stream_cancellation(&engine, &request_id)?;

    let active_context = match active_engine_context(&engine, &requested_model_id) {
        Ok(context) => context,
        Err(err) => {
            unregister_stream_cancellation(&engine, &request_id);
            return Err(err);
        }
    };
    let process_label = engine_process_label(&active_context.backend);

    match probe_engine_health(
        &active_context.host,
        active_context.port,
        HTTP_HEALTH_TIMEOUT_MS,
    ) {
        HealthProbe::Ready => {}
        HealthProbe::Loading(message) => {
            unregister_stream_cancellation(&engine, &request_id);
            return Err(format!("{process_label} is still loading: {message}"));
        }
        HealthProbe::Offline(message) => {
            unregister_stream_cancellation(&engine, &request_id);
            return Err(format!("{process_label} is not reachable: {message}"));
        }
    }

    let started = std::time::Instant::now();
    let mut payload = match build_chat_completion_payload(
        &profile,
        &active_context.request_model_name,
        prompt,
        &history,
    ) {
        Ok(payload) => payload,
        Err(err) => {
            unregister_stream_cancellation(&engine, &request_id);
            return Err(err);
        }
    };
    payload["stream"] = json!(true);

    if let Err(err) = emit_stream_event(
        &window,
        InferenceStreamEvent {
            request_id: request_id.clone(),
            phase: "started".to_string(),
            delta: String::new(),
            content: String::new(),
            model: active_context.active_model_name.clone(),
            completion_tokens: 0,
            tokens_per_second: 0.0,
            elapsed_ms: 0,
            finish_reason: None,
        },
    ) {
        unregister_stream_cancellation(&engine, &request_id);
        return Err(err);
    }

    let mut accumulator = StreamAccumulator::default();
    let stream_result = http_sse_request(
        &active_context.host,
        active_context.port,
        "POST",
        "/v1/chat/completions",
        &payload,
        HTTP_CHAT_TIMEOUT_MS,
        &cancel_token,
        |value| {
            let chunk = extract_stream_chunk(&value);
            if !chunk.delta.is_empty() {
                accumulator.content.push_str(&chunk.delta);
                accumulator.completion_tokens = accumulator.completion_tokens.saturating_add(1);
                let elapsed_ms = started.elapsed().as_millis();
                let tokens_per_second =
                    tokens_per_second(accumulator.completion_tokens, elapsed_ms);
                emit_stream_event(
                    &window,
                    InferenceStreamEvent {
                        request_id: request_id.clone(),
                        phase: "delta".to_string(),
                        delta: chunk.delta,
                        content: accumulator.content.clone(),
                        model: active_context.active_model_name.clone(),
                        completion_tokens: accumulator.completion_tokens,
                        tokens_per_second,
                        elapsed_ms,
                        finish_reason: None,
                    },
                )?;
            }

            if chunk.finish_reason.is_some() {
                accumulator.finish_reason = chunk.finish_reason;
            }
            Ok(!chunk.stop)
        },
    );

    if let Err(err) = stream_result {
        let was_cancelled = cancel_token.load(Ordering::SeqCst);
        let phase = if was_cancelled { "cancelled" } else { "error" };
        let emit_result = emit_stream_event(
            &window,
            InferenceStreamEvent {
                request_id: request_id.clone(),
                phase: phase.to_string(),
                delta: String::new(),
                content: accumulator.content,
                model: active_context.active_model_name.clone(),
                completion_tokens: accumulator.completion_tokens,
                tokens_per_second: 0.0,
                elapsed_ms: started.elapsed().as_millis(),
                finish_reason: Some(if was_cancelled {
                    "cancelled".to_string()
                } else {
                    err.clone()
                }),
            },
        );
        unregister_stream_cancellation(&engine, &request_id);
        emit_result?;
        if was_cancelled {
            return Err("stream cancelled".to_string());
        }
        return Err(err);
    }

    if accumulator.content.is_empty() {
        unregister_stream_cancellation(&engine, &request_id);
        return Err(format!(
            "{process_label} returned an empty streamed completion"
        ));
    }

    let elapsed_ms = started.elapsed().as_millis();
    let completion_tokens = Some(accumulator.completion_tokens);
    let tokens_per_second = tokens_per_second(accumulator.completion_tokens, elapsed_ms);
    let total_tokens = completion_tokens;

    {
        let mut guard = engine
            .inner
            .lock()
            .map_err(|_| "engine state lock is poisoned".to_string())?;
        guard.last_tokens_per_second = tokens_per_second;
        guard.context_used_tokens = total_tokens
            .unwrap_or_default()
            .min(profile.runtime.context_length);
        guard.context_total_tokens = profile.runtime.context_length;
        guard.active_model_id = Some(active_context.active_model_id.clone());
        guard.active_model_name = Some(active_context.active_model_name.clone());
        guard.last_error = None;
    }

    let emit_result = emit_stream_event(
        &window,
        InferenceStreamEvent {
            request_id: request_id.clone(),
            phase: "done".to_string(),
            delta: String::new(),
            content: accumulator.content.clone(),
            model: active_context.active_model_name.clone(),
            completion_tokens: accumulator.completion_tokens,
            tokens_per_second,
            elapsed_ms,
            finish_reason: accumulator.finish_reason.clone(),
        },
    );
    unregister_stream_cancellation(&engine, &request_id);
    emit_result?;

    Ok(InferenceRunResult {
        content: accumulator.content,
        model: active_context.active_model_name,
        backend: active_context.backend,
        elapsed_ms,
        tokens_per_second,
        prompt_tokens: None,
        completion_tokens,
        total_tokens,
        finish_reason: accumulator.finish_reason,
    })
}

#[tauri::command]
fn get_api_settings(app_handle: AppHandle) -> Result<ApiSettings, String> {
    read_api_settings(&app_handle)
}

#[tauri::command]
fn save_api_settings(
    app_handle: AppHandle,
    engine: State<'_, EngineRuntime>,
    settings: ApiSettings,
) -> Result<ApiStatus, String> {
    let settings = normalize_api_settings(settings)?;

    {
        let mut guard = engine
            .inner
            .lock()
            .map_err(|_| "engine state lock is poisoned".to_string())?;
        refresh_engine_process(&mut guard);
        let endpoint_changed = guard.host != settings.host || guard.port != settings.port;
        if guard.child.is_some() && endpoint_changed {
            return Err("Stop the running API server before changing host or port.".to_string());
        }
        guard.host = settings.host.clone();
        guard.port = settings.port;
    }

    write_api_settings(&app_handle, &settings)?;
    let _ = append_system_log(
        &app_handle,
        "INFO",
        "api",
        format!(
            "Saved local API endpoint {}:{}",
            settings.host, settings.port
        ),
    );
    get_api_status(app_handle, engine)
}

#[tauri::command]
fn get_api_status(
    app_handle: AppHandle,
    engine: State<'_, EngineRuntime>,
) -> Result<ApiStatus, String> {
    let settings = read_api_settings(&app_handle)?;
    sync_engine_endpoint_if_idle(&engine, &settings)?;
    let status = current_engine_status(&engine)?;

    Ok(ApiStatus {
        enabled: matches!(status.state.as_str(), "ready" | "loading"),
        port: status.port,
        base_url: status.base_url,
        endpoints: vec![
            ApiEndpoint {
                method: "POST".to_string(),
                path: "/v1/chat/completions".to_string(),
                description: format!(
                    "OpenAI-compatible local chat completions through {}",
                    status.backend
                ),
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
                description: "Local engine readiness probe".to_string(),
                status: status.state,
            },
        ],
    })
}

#[tauri::command]
fn list_benchmark_results(app_handle: AppHandle) -> Result<Vec<BenchmarkResult>, String> {
    read_benchmark_results(&app_handle)
}

#[tauri::command]
fn run_benchmark(
    app_handle: AppHandle,
    engine: State<'_, EngineRuntime>,
    model_id: String,
    profile: InferenceProfile,
) -> Result<Vec<BenchmarkResult>, String> {
    let mut benchmark_profile = profile;
    benchmark_profile.sampling.temperature = 0.0;
    benchmark_profile.sampling.seed = Some(42);
    benchmark_profile.sampling.max_tokens = 128;
    benchmark_profile.sampling.stop_sequences.clear();
    benchmark_profile.output.mode = "text".to_string();
    benchmark_profile.output.json_schema.clear();
    benchmark_profile.output.grammar.clear();
    benchmark_profile.output.logprobs = false;
    benchmark_profile.output.top_logprobs = 0;

    let completion = run_chat_completion_inner(
        &engine,
        model_id,
        benchmark_profile,
        benchmark_prompt().to_string(),
        Vec::new(),
    )?;
    let elapsed_ms = u64::try_from(completion.elapsed_ms).unwrap_or(u64::MAX);
    let eval_count = completion
        .completion_tokens
        .unwrap_or_else(|| estimate_token_count(&completion.content));
    let tokens_per_second = if completion.tokens_per_second > 0.0 {
        completion.tokens_per_second
    } else {
        tokens_per_second(eval_count, completion.elapsed_ms)
    };
    let load_duration_ms = current_load_duration_ms(&engine)?;
    let mut results = read_benchmark_results(&app_handle)?;
    results.insert(
        0,
        BenchmarkResult {
            model: completion.model,
            backend: completion.backend,
            eval_count,
            eval_duration_ms: elapsed_ms,
            tokens_per_second,
            load_duration_ms,
        },
    );
    results.truncate(MAX_BENCHMARK_RESULTS);
    write_benchmark_results(&app_handle, &results)?;
    if let Some(result) = results.first() {
        let _ = append_system_log(
            &app_handle,
            "INFO",
            "benchmarks",
            format!(
                "Recorded benchmark for {} at {:.1} tok/s",
                result.model, result.tokens_per_second
            ),
        );
    }

    Ok(results)
}

#[tauri::command]
fn list_knowledge_bases(app_handle: AppHandle) -> Result<Vec<KnowledgeBase>, String> {
    let mut store = read_knowledge_store(&app_handle)?;
    ensure_default_knowledge_base(&mut store);
    write_knowledge_store(&app_handle, &store)?;

    Ok(store.bases)
}

#[tauri::command]
fn create_knowledge_base(
    app_handle: AppHandle,
    name: String,
) -> Result<Vec<KnowledgeBase>, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("knowledge base name is required".to_string());
    }
    let mut store = read_knowledge_store(&app_handle)?;
    let id = unique_knowledge_base_id(&store, name);
    let timestamp = unix_timestamp().to_string();
    store.bases.push(KnowledgeBase {
        id,
        name: name.to_string(),
        document_count: 0,
        chunk_count: 0,
        updated_at: timestamp,
    });
    sort_knowledge_bases(&mut store);
    write_knowledge_store(&app_handle, &store)?;
    let _ = append_system_log(
        &app_handle,
        "INFO",
        "knowledge",
        format!("Created knowledge base {name}"),
    );

    Ok(store.bases)
}

#[tauri::command]
fn list_knowledge_documents(
    app_handle: AppHandle,
    knowledge_base_id: String,
) -> Result<Vec<KnowledgeDocument>, String> {
    let store = read_knowledge_store(&app_handle)?;
    let knowledge_base_id = sanitize_identifier(&knowledge_base_id);
    Ok(documents_for_base(&store, &knowledge_base_id))
}

#[tauri::command]
fn import_knowledge_document(
    app_handle: AppHandle,
    knowledge_base_id: String,
    path: String,
) -> Result<KnowledgeBaseDetail, String> {
    let knowledge_base_id = sanitize_identifier(&knowledge_base_id);
    if knowledge_base_id.is_empty() {
        return Err("knowledge base id is required".to_string());
    }
    let mut store = read_knowledge_store(&app_handle)?;
    ensure_default_knowledge_base(&mut store);
    if !store.bases.iter().any(|base| base.id == knowledge_base_id) {
        return Err(format!("knowledge base not found: {knowledge_base_id}"));
    }

    let document_path = canonical_knowledge_document_path(&path)?;
    let metadata = fs::metadata(&document_path).map_err(|err| {
        format!(
            "failed to inspect document {}: {err}",
            document_path.display()
        )
    })?;
    if metadata.len() > MAX_KNOWLEDGE_DOCUMENT_BYTES {
        return Err(format!(
            "document exceeds {} MiB import limit",
            MAX_KNOWLEDGE_DOCUMENT_BYTES / 1024 / 1024
        ));
    }

    let raw = fs::read_to_string(&document_path).map_err(|err| {
        format!(
            "failed to read UTF-8 document {}: {err}",
            document_path.display()
        )
    })?;
    let chunks = chunk_document_text(&raw);
    if chunks.is_empty() {
        return Err("document did not contain indexable text".to_string());
    }
    let chunk_count = chunks.len();

    let document_name = document_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("document")
        .to_string();
    let document_id = format!(
        "{}-{}",
        sanitize_identifier(&document_name),
        unix_timestamp()
    );
    let document_path_string = document_path.to_string_lossy().to_string();
    let replaced_document_ids = store
        .documents
        .iter()
        .filter(|document| {
            document.knowledge_base_id == knowledge_base_id && document.path == document_path_string
        })
        .map(|document| document.id.clone())
        .collect::<Vec<_>>();
    store
        .documents
        .retain(|document| !replaced_document_ids.iter().any(|id| id == &document.id));
    store.chunks.retain(|chunk| {
        !replaced_document_ids
            .iter()
            .any(|document_id| document_id == &chunk.document_id)
    });

    let imported_at = unix_timestamp().to_string();
    store.documents.push(KnowledgeDocument {
        id: document_id.clone(),
        knowledge_base_id: knowledge_base_id.clone(),
        name: document_name.clone(),
        path: document_path_string,
        size_bytes: metadata.len(),
        chunk_count: chunk_count as u32,
        imported_at,
    });
    for (index, content) in chunks.into_iter().enumerate() {
        store.chunks.push(KnowledgeChunk {
            id: format!("{document_id}-{index}"),
            knowledge_base_id: knowledge_base_id.clone(),
            document_id: document_id.clone(),
            document_name: document_name.clone(),
            chunk_index: index as u32,
            content,
        });
    }
    refresh_knowledge_base_counts(&mut store, &knowledge_base_id);
    sort_knowledge_bases(&mut store);
    write_knowledge_store(&app_handle, &store)?;
    let _ = append_system_log(
        &app_handle,
        "INFO",
        "knowledge",
        format!("Imported document {document_name} into {chunk_count} chunks"),
    );

    knowledge_base_detail(&store, &knowledge_base_id)
}

#[tauri::command]
fn test_knowledge_retrieval(
    app_handle: AppHandle,
    knowledge_base_id: String,
    query: String,
) -> Result<Vec<RetrievalMatch>, String> {
    let knowledge_base_id = sanitize_identifier(&knowledge_base_id);
    let query_terms = tokenize_for_retrieval(&query);
    if query_terms.is_empty() {
        return Err("retrieval query is required".to_string());
    }
    let store = read_knowledge_store(&app_handle)?;
    if !store.bases.iter().any(|base| base.id == knowledge_base_id) {
        return Err(format!("knowledge base not found: {knowledge_base_id}"));
    }

    Ok(rank_retrieval_matches(
        &store,
        &knowledge_base_id,
        &query_terms,
    ))
}

#[tauri::command]
fn list_system_logs(app_handle: AppHandle) -> Result<Vec<LogEntry>, String> {
    let mut logs = read_system_logs(&app_handle)?;
    if logs.is_empty() {
        logs = seeded_system_logs();
        write_system_logs(&app_handle, &logs)?;
    }

    Ok(logs)
}

fn profile_directory(app_handle: &AppHandle) -> Result<PathBuf, String> {
    app_handle
        .path()
        .app_config_dir()
        .map(|path| path.join("profiles"))
        .map_err(|err| format!("failed to resolve app profile directory: {err}"))
}

fn system_logs_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    app_handle
        .path()
        .app_config_dir()
        .map(|path| path.join(SYSTEM_LOG_FILE))
        .map_err(|err| format!("failed to resolve system log path: {err}"))
}

fn read_system_logs(app_handle: &AppHandle) -> Result<Vec<LogEntry>, String> {
    let path = system_logs_path(app_handle)?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    let raw = fs::read_to_string(&path)
        .map_err(|err| format!("failed to read system logs {}: {err}", path.display()))?;
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }

    let mut logs = serde_json::from_str::<Vec<LogEntry>>(&raw)
        .map_err(|err| format!("failed to parse system logs {}: {err}", path.display()))?;
    logs.truncate(MAX_SYSTEM_LOGS);

    Ok(logs)
}

fn write_system_logs(app_handle: &AppHandle, logs: &[LogEntry]) -> Result<(), String> {
    let path = system_logs_path(app_handle)?;
    let Some(directory) = path.parent() else {
        return Err("system log path has no parent directory".to_string());
    };
    fs::create_dir_all(directory).map_err(|err| {
        format!(
            "failed to create system log directory {}: {err}",
            directory.display()
        )
    })?;
    let encoded = serde_json::to_string_pretty(logs)
        .map_err(|err| format!("failed to encode system logs: {err}"))?;
    fs::write(&path, encoded)
        .map_err(|err| format!("failed to write system logs {}: {err}", path.display()))
}

fn append_system_log(
    app_handle: &AppHandle,
    level: &str,
    source: &str,
    message: impl Into<String>,
) -> Result<(), String> {
    let mut logs = read_system_logs(app_handle)?;
    if logs.is_empty() {
        logs = seeded_system_logs();
    }
    logs.insert(
        0,
        LogEntry {
            level: level.to_ascii_uppercase(),
            source: source.to_string(),
            message: message.into(),
            timestamp: unix_timestamp().to_string(),
        },
    );
    logs.truncate(MAX_SYSTEM_LOGS);
    write_system_logs(app_handle, &logs)
}

fn seeded_system_logs() -> Vec<LogEntry> {
    vec![
        LogEntry {
            level: "INFO".to_string(),
            source: "core".to_string(),
            message: "Kivarro desktop shell initialized".to_string(),
            timestamp: unix_timestamp().to_string(),
        },
        LogEntry {
            level: "INFO".to_string(),
            source: "registry".to_string(),
            message: "Model discovery scans and imports local model files under ./models"
                .to_string(),
            timestamp: unix_timestamp().to_string(),
        },
        LogEntry {
            level: "INFO".to_string(),
            source: "api".to_string(),
            message: "Local API endpoint settings are persisted in the app config directory"
                .to_string(),
            timestamp: unix_timestamp().to_string(),
        },
    ]
}

fn api_settings_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    app_handle
        .path()
        .app_config_dir()
        .map(|path| path.join(API_SETTINGS_FILE))
        .map_err(|err| format!("failed to resolve API settings path: {err}"))
}

fn default_api_settings() -> ApiSettings {
    ApiSettings {
        host: DEFAULT_API_HOST.to_string(),
        port: configured_api_port(),
    }
}

fn read_api_settings(app_handle: &AppHandle) -> Result<ApiSettings, String> {
    let path = api_settings_path(app_handle)?;
    if !path.exists() {
        return Ok(default_api_settings());
    }

    let raw = fs::read_to_string(&path)
        .map_err(|err| format!("failed to read API settings {}: {err}", path.display()))?;
    if raw.trim().is_empty() {
        return Ok(default_api_settings());
    }

    let settings = serde_json::from_str::<ApiSettings>(&raw)
        .map_err(|err| format!("failed to parse API settings {}: {err}", path.display()))?;
    normalize_api_settings(settings)
}

fn write_api_settings(app_handle: &AppHandle, settings: &ApiSettings) -> Result<(), String> {
    let path = api_settings_path(app_handle)?;
    let Some(directory) = path.parent() else {
        return Err("API settings path has no parent directory".to_string());
    };
    fs::create_dir_all(directory).map_err(|err| {
        format!(
            "failed to create API settings directory {}: {err}",
            directory.display()
        )
    })?;
    let encoded = serde_json::to_string_pretty(settings)
        .map_err(|err| format!("failed to encode API settings: {err}"))?;
    fs::write(&path, encoded)
        .map_err(|err| format!("failed to write API settings {}: {err}", path.display()))
}

fn normalize_api_settings(settings: ApiSettings) -> Result<ApiSettings, String> {
    let host = settings.host.trim();
    if host.is_empty() {
        return Err("API host is required".to_string());
    }
    if host.chars().any(char::is_whitespace) {
        return Err("API host cannot contain whitespace".to_string());
    }
    if settings.port == 0 {
        return Err("API port must be between 1 and 65535".to_string());
    }

    Ok(ApiSettings {
        host: host.to_string(),
        port: settings.port,
    })
}

fn sync_engine_endpoint_if_idle(
    engine: &State<'_, EngineRuntime>,
    settings: &ApiSettings,
) -> Result<(), String> {
    let mut guard = engine
        .inner
        .lock()
        .map_err(|_| "engine state lock is poisoned".to_string())?;
    refresh_engine_process(&mut guard);
    if guard.child.is_none() {
        guard.host = settings.host.clone();
        guard.port = settings.port;
    }

    Ok(())
}

fn api_base_url(host: &str, port: u16) -> String {
    format!("http://{host}:{port}/v1")
}

fn benchmark_results_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    app_handle
        .path()
        .app_config_dir()
        .map(|path| path.join(BENCHMARK_RESULTS_FILE))
        .map_err(|err| format!("failed to resolve benchmark results path: {err}"))
}

fn read_benchmark_results(app_handle: &AppHandle) -> Result<Vec<BenchmarkResult>, String> {
    let path = benchmark_results_path(app_handle)?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    let raw = fs::read_to_string(&path)
        .map_err(|err| format!("failed to read benchmark results {}: {err}", path.display()))?;
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }

    let mut results = serde_json::from_str::<Vec<BenchmarkResult>>(&raw).map_err(|err| {
        format!(
            "failed to parse benchmark results {}: {err}",
            path.display()
        )
    })?;
    results.truncate(MAX_BENCHMARK_RESULTS);

    Ok(results)
}

fn write_benchmark_results(
    app_handle: &AppHandle,
    results: &[BenchmarkResult],
) -> Result<(), String> {
    let path = benchmark_results_path(app_handle)?;
    let Some(directory) = path.parent() else {
        return Err("benchmark results path has no parent directory".to_string());
    };
    fs::create_dir_all(directory).map_err(|err| {
        format!(
            "failed to create benchmark directory {}: {err}",
            directory.display()
        )
    })?;
    let encoded = serde_json::to_string_pretty(results)
        .map_err(|err| format!("failed to encode benchmark results: {err}"))?;
    fs::write(&path, encoded).map_err(|err| {
        format!(
            "failed to write benchmark results {}: {err}",
            path.display()
        )
    })
}

fn benchmark_prompt() -> &'static str {
    "Benchmark local inference throughput. Write one dense technical paragraph about memory bandwidth, context windows, GPU offload, and token sampling. Continue until the response is complete."
}

fn current_load_duration_ms(engine: &State<'_, EngineRuntime>) -> Result<u64, String> {
    let guard = engine
        .inner
        .lock()
        .map_err(|_| "engine state lock is poisoned".to_string())?;

    Ok(guard.last_load_duration_ms)
}

fn estimate_token_count(content: &str) -> u32 {
    let words = content.split_whitespace().count();
    u32::try_from(words.max(1)).unwrap_or(u32::MAX)
}

fn knowledge_store_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    app_handle
        .path()
        .app_config_dir()
        .map(|path| path.join(KNOWLEDGE_STORE_FILE))
        .map_err(|err| format!("failed to resolve knowledge store path: {err}"))
}

fn read_knowledge_store(app_handle: &AppHandle) -> Result<KnowledgeStore, String> {
    let path = knowledge_store_path(app_handle)?;
    if !path.exists() {
        return Ok(KnowledgeStore::default());
    }

    let raw = fs::read_to_string(&path)
        .map_err(|err| format!("failed to read knowledge store {}: {err}", path.display()))?;
    if raw.trim().is_empty() {
        return Ok(KnowledgeStore::default());
    }

    serde_json::from_str::<KnowledgeStore>(&raw)
        .map_err(|err| format!("failed to parse knowledge store {}: {err}", path.display()))
}

fn write_knowledge_store(app_handle: &AppHandle, store: &KnowledgeStore) -> Result<(), String> {
    let path = knowledge_store_path(app_handle)?;
    let Some(directory) = path.parent() else {
        return Err("knowledge store path has no parent directory".to_string());
    };
    fs::create_dir_all(directory).map_err(|err| {
        format!(
            "failed to create knowledge store directory {}: {err}",
            directory.display()
        )
    })?;
    let encoded = serde_json::to_string_pretty(store)
        .map_err(|err| format!("failed to encode knowledge store: {err}"))?;
    fs::write(&path, encoded)
        .map_err(|err| format!("failed to write knowledge store {}: {err}", path.display()))
}

fn ensure_default_knowledge_base(store: &mut KnowledgeStore) {
    if !store.bases.is_empty() {
        return;
    }

    store.bases.push(KnowledgeBase {
        id: "research-vault".to_string(),
        name: "Research Vault".to_string(),
        document_count: 0,
        chunk_count: 0,
        updated_at: unix_timestamp().to_string(),
    });
}

fn unique_knowledge_base_id(store: &KnowledgeStore, name: &str) -> String {
    let base = sanitize_identifier(name);
    let base = if base.is_empty() {
        "knowledge-base".to_string()
    } else {
        base
    };
    if !store.bases.iter().any(|candidate| candidate.id == base) {
        return base;
    }

    let mut index = 2;
    loop {
        let candidate = format!("{base}-{index}");
        if !store.bases.iter().any(|base| base.id == candidate) {
            return candidate;
        }
        index += 1;
    }
}

fn sort_knowledge_bases(store: &mut KnowledgeStore) {
    store
        .bases
        .sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
}

fn documents_for_base(store: &KnowledgeStore, knowledge_base_id: &str) -> Vec<KnowledgeDocument> {
    let mut documents = store
        .documents
        .iter()
        .filter(|document| document.knowledge_base_id == knowledge_base_id)
        .cloned()
        .collect::<Vec<_>>();
    documents.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    documents
}

fn knowledge_base_detail(
    store: &KnowledgeStore,
    knowledge_base_id: &str,
) -> Result<KnowledgeBaseDetail, String> {
    let base = store
        .bases
        .iter()
        .find(|base| base.id == knowledge_base_id)
        .cloned()
        .ok_or_else(|| format!("knowledge base not found: {knowledge_base_id}"))?;
    Ok(KnowledgeBaseDetail {
        base,
        documents: documents_for_base(store, knowledge_base_id),
    })
}

fn refresh_knowledge_base_counts(store: &mut KnowledgeStore, knowledge_base_id: &str) {
    let document_count = store
        .documents
        .iter()
        .filter(|document| document.knowledge_base_id == knowledge_base_id)
        .count() as u32;
    let chunk_count = store
        .chunks
        .iter()
        .filter(|chunk| chunk.knowledge_base_id == knowledge_base_id)
        .count() as u32;
    if let Some(base) = store
        .bases
        .iter_mut()
        .find(|base| base.id == knowledge_base_id)
    {
        base.document_count = document_count;
        base.chunk_count = chunk_count;
        base.updated_at = unix_timestamp().to_string();
    }
}

fn canonical_knowledge_document_path(path: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(path.trim().trim_matches('"'));
    if !path.exists() {
        return Err(format!("document does not exist: {}", path.display()));
    }
    if !path.is_file() {
        return Err(format!("document path is not a file: {}", path.display()));
    }

    path.canonicalize()
        .map_err(|err| format!("failed to resolve document path {}: {err}", path.display()))
}

fn chunk_document_text(raw: &str) -> Vec<String> {
    let normalized = raw
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if normalized.is_empty() {
        return Vec::new();
    }

    let chars = normalized.chars().collect::<Vec<_>>();
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let mut end = (start + KNOWLEDGE_CHUNK_TARGET_CHARS).min(chars.len());
        if end < chars.len() {
            if let Some(boundary) = chars[start..end]
                .iter()
                .rposition(|ch| matches!(ch, '.' | '!' | '?' | '\n'))
            {
                end = (start + boundary + 1).max(start + 1);
            }
        }
        let content = chars[start..end]
            .iter()
            .collect::<String>()
            .trim()
            .to_string();
        if !content.is_empty() {
            chunks.push(content);
        }
        if end >= chars.len() {
            break;
        }
        start = end
            .saturating_sub(KNOWLEDGE_CHUNK_OVERLAP_CHARS)
            .max(start + 1);
    }

    chunks
}

fn tokenize_for_retrieval(input: &str) -> Vec<String> {
    input
        .split(|ch: char| !ch.is_alphanumeric())
        .map(|token| token.trim().to_ascii_lowercase())
        .filter(|token| token.len() >= 3)
        .collect()
}

fn rank_retrieval_matches(
    store: &KnowledgeStore,
    knowledge_base_id: &str,
    query_terms: &[String],
) -> Vec<RetrievalMatch> {
    let mut matches = store
        .chunks
        .iter()
        .filter(|chunk| chunk.knowledge_base_id == knowledge_base_id)
        .filter_map(|chunk| {
            let chunk_terms = tokenize_for_retrieval(&chunk.content);
            let score = retrieval_score(query_terms, &chunk_terms);
            if score <= 0.0 {
                return None;
            }
            Some(RetrievalMatch {
                knowledge_base_id: chunk.knowledge_base_id.clone(),
                document_id: chunk.document_id.clone(),
                document_name: chunk.document_name.clone(),
                chunk_index: chunk.chunk_index,
                score,
                snippet: retrieval_snippet(&chunk.content, query_terms),
            })
        })
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    matches.truncate(MAX_RETRIEVAL_RESULTS);

    matches
}

fn retrieval_score(query_terms: &[String], chunk_terms: &[String]) -> f32 {
    if query_terms.is_empty() || chunk_terms.is_empty() {
        return 0.0;
    }
    let mut hits = 0_u32;
    for query in query_terms {
        if chunk_terms.iter().any(|term| term == query) {
            hits += 2;
        } else if chunk_terms.iter().any(|term| term.contains(query)) {
            hits += 1;
        }
    }

    hits as f32 / ((query_terms.len() as f32) * 2.0)
}

fn retrieval_snippet(content: &str, query_terms: &[String]) -> String {
    let lower = content.to_ascii_lowercase();
    let first_hit = query_terms
        .iter()
        .filter_map(|term| lower.find(term))
        .min()
        .unwrap_or(0);
    let start = first_hit.saturating_sub(120);
    let end = (first_hit + 360).min(content.len());
    let mut snippet = content
        .get(start..end)
        .unwrap_or(content)
        .replace('\n', " ")
        .trim()
        .to_string();
    if start > 0 {
        snippet.insert_str(0, "...");
    }
    if end < content.len() {
        snippet.push_str("...");
    }

    snippet
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
    profile.runtime.backend = canonical_backend(&profile.runtime.backend)
        .unwrap_or(BACKEND_LLAMA_CPP)
        .to_string();
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
    profile_backend(profile)?;
    if profile.output.top_logprobs > 20 {
        return Err("top_logprobs must be 20 or lower".to_string());
    }
    Ok(())
}

fn canonical_backend(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "llama" | "llama.cpp" | "llama-cpp" | "llamacpp" => Some(BACKEND_LLAMA_CPP),
        "mistral" | "mistral.rs" | "mistral-rs" | "mistralrs" => Some(BACKEND_MISTRAL_RS),
        _ => None,
    }
}

fn profile_backend(profile: &InferenceProfile) -> Result<&'static str, String> {
    canonical_backend(&profile.runtime.backend).ok_or_else(|| {
        format!(
            "unsupported backend '{}'. Choose llama.cpp or mistral.rs.",
            profile.runtime.backend
        )
    })
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

fn register_stream_cancellation(
    engine: &State<'_, EngineRuntime>,
    request_id: &str,
) -> Result<Arc<AtomicBool>, String> {
    let token = Arc::new(AtomicBool::new(false));
    let mut guard = engine
        .stream_cancellations
        .lock()
        .map_err(|_| "stream cancellation registry lock is poisoned".to_string())?;
    guard.insert(request_id.to_string(), Arc::clone(&token));
    Ok(token)
}

fn unregister_stream_cancellation(engine: &State<'_, EngineRuntime>, request_id: &str) {
    if let Ok(mut guard) = engine.stream_cancellations.lock() {
        guard.remove(request_id);
    }
}

fn active_engine_context(
    engine: &State<'_, EngineRuntime>,
    requested_model_id: &str,
) -> Result<ActiveEngineContext, String> {
    let mut guard = engine
        .inner
        .lock()
        .map_err(|_| "engine state lock is poisoned".to_string())?;
    refresh_engine_process(&mut guard);

    let backend = guard.active_backend.clone();
    let process_label = engine_process_label(&backend);
    let active_model_id = guard
        .active_model_id
        .clone()
        .ok_or_else(|| format!("no {process_label} model is loaded"))?;
    if active_model_id != requested_model_id {
        return Err(format!(
            "selected model is not the active {} model",
            backend
        ));
    }
    if guard.child.is_none() {
        return Err(format!("{process_label} is not running"));
    }

    let active_model_name = guard
        .active_model_name
        .clone()
        .unwrap_or_else(|| "local-model".to_string());
    let request_model_name = guard
        .active_request_model
        .clone()
        .unwrap_or_else(|| request_model_name(&backend, &active_model_name));

    Ok(ActiveEngineContext {
        host: guard.host.clone(),
        port: guard.port,
        backend,
        active_model_id,
        active_model_name,
        request_model_name,
    })
}

fn current_engine_status(engine: &State<'_, EngineRuntime>) -> Result<EngineStatus, String> {
    let mut guard = engine
        .inner
        .lock()
        .map_err(|_| "engine state lock is poisoned".to_string())?;
    let binary_path = find_engine_binary(&guard.active_backend);

    Ok(engine_status_from_guard(&mut guard, binary_path))
}

fn engine_status_from_guard(
    guard: &mut ManagedEngine,
    binary_path: Option<PathBuf>,
) -> EngineStatus {
    refresh_engine_process(guard);

    let pid = guard.child.as_ref().map(Child::id);
    let backend = guard.active_backend.clone();
    let process_label = engine_process_label(&backend);
    let (state, message, health_ok) = if guard.child.is_some() {
        match probe_engine_health(&guard.host, guard.port, HTTP_HEALTH_TIMEOUT_MS) {
            HealthProbe::Ready => {
                if let Some(started_at) = guard.load_started_at.take() {
                    guard.last_load_duration_ms = started_at.elapsed().as_millis() as u64;
                }
                (
                    "ready".to_string(),
                    format!("{process_label} is ready for local chat completions"),
                    true,
                )
            }
            HealthProbe::Loading(message) => ("loading".to_string(), message, false),
            HealthProbe::Offline(message) => ("loading".to_string(), message, false),
        }
    } else if let Some(error) = guard.last_error.as_ref() {
        ("error".to_string(), error.clone(), false)
    } else if binary_path.is_some() {
        (
            "offline".to_string(),
            format!("{process_label} is configured but no model is loaded"),
            false,
        )
    } else {
        (
            "unconfigured".to_string(),
            engine_binary_missing_message(&backend),
            false,
        )
    };

    EngineStatus {
        backend,
        state,
        message,
        configured: binary_path.is_some(),
        binary_path: binary_path.map(|path| path.to_string_lossy().to_string()),
        pid,
        active_model_id: guard.active_model_id.clone(),
        active_model_name: guard.active_model_name.clone(),
        host: guard.host.clone(),
        port: guard.port,
        base_url: api_base_url(&guard.host, guard.port),
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
            guard.load_started_at = None;
            guard.last_error = Some(format!(
                "{} exited with status {status}",
                engine_process_label(&guard.active_backend)
            ));
        }
        Ok(None) => {}
        Err(err) => {
            guard.child = None;
            guard.load_started_at = None;
            guard.last_error = Some(format!(
                "failed to inspect {} process: {err}",
                engine_process_label(&guard.active_backend)
            ));
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
            let process_label = engine_process_label(&guard.active_backend);
            child
                .kill()
                .map_err(|err| format!("failed to stop {process_label}: {err}"))?;
            child
                .wait()
                .map(|_| ())
                .map_err(|err| format!("failed to wait for {process_label} shutdown: {err}"))
        }
        Err(err) => Err(format!(
            "failed to inspect {} before stop: {err}",
            engine_process_label(&guard.active_backend)
        )),
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

fn find_engine_binary(backend: &str) -> Option<PathBuf> {
    match canonical_backend(backend).unwrap_or(BACKEND_LLAMA_CPP) {
        BACKEND_MISTRAL_RS => find_mistralrs_binary(),
        _ => find_llama_server_binary(),
    }
}

fn engine_process_label(backend: &str) -> &'static str {
    match canonical_backend(backend).unwrap_or(BACKEND_LLAMA_CPP) {
        BACKEND_MISTRAL_RS => "mistralrs serve",
        _ => "llama-server",
    }
}

fn engine_binary_missing_message(backend: &str) -> String {
    match canonical_backend(backend).unwrap_or(BACKEND_LLAMA_CPP) {
        BACKEND_MISTRAL_RS => {
            format!("mistralrs not found. Set {MISTRALRS_ENV} to the binary path or add mistralrs to PATH.")
        }
        _ => format!(
            "llama-server not found. Set {LLAMA_SERVER_ENV} to the binary path or add llama-server to PATH."
        ),
    }
}

fn request_model_name(backend: &str, display_name: &str) -> String {
    match canonical_backend(backend).unwrap_or(BACKEND_LLAMA_CPP) {
        BACKEND_MISTRAL_RS => "default".to_string(),
        _ => display_name.to_string(),
    }
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

fn find_mistralrs_binary() -> Option<PathBuf> {
    if let Ok(value) = env::var(MISTRALRS_ENV) {
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

    [
        "mistralrs",
        "mistralrs.exe",
        "mistralrs-server",
        "mistralrs-server.exe",
    ]
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

fn build_engine_args(
    backend: &str,
    model_path: &Path,
    profile: &InferenceProfile,
    host: &str,
    port: u16,
) -> Vec<String> {
    match canonical_backend(backend).unwrap_or(BACKEND_LLAMA_CPP) {
        BACKEND_MISTRAL_RS => build_mistralrs_serve_args(model_path, host, port),
        _ => build_llama_server_args(model_path, profile, host, port),
    }
}

fn build_mistralrs_serve_args(model_path: &Path, host: &str, port: u16) -> Vec<String> {
    vec![
        "serve".to_string(),
        "-m".to_string(),
        model_path.to_string_lossy().to_string(),
        "--host".to_string(),
        host.to_string(),
        "-p".to_string(),
        port.to_string(),
        "--no-ui".to_string(),
    ]
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

fn probe_engine_health(host: &str, port: u16, timeout_ms: u64) -> HealthProbe {
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
        .map_err(|err| format!("invalid local engine address {host}:{port}: {err}"))?;
    let timeout = std::time::Duration::from_millis(timeout_ms);
    let mut stream = TcpStream::connect_timeout(&address, timeout)
        .map_err(|err| format!("failed to connect to local engine at {host}:{port}: {err}"))?;
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
            "local engine response exceeds {} bytes",
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

fn parse_openai_json_response(
    response: HttpResponse,
    process_label: &str,
) -> Result<Value, String> {
    let body = serde_json::from_slice::<Value>(&response.body)
        .map_err(|err| format!("{process_label} returned invalid JSON: {err}"))?;
    if response.status_code >= 400 {
        let message = body
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or("local engine request failed");
        return Err(format!(
            "{process_label} HTTP {}: {message}",
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

fn emit_stream_event(window: &Window, payload: InferenceStreamEvent) -> Result<(), String> {
    window
        .emit(STREAM_EVENT_NAME, payload)
        .map_err(|err| format!("failed to emit stream event: {err}"))
}

fn tokens_per_second(tokens: u32, elapsed_ms: u128) -> f32 {
    if tokens == 0 || elapsed_ms == 0 {
        return 0.0;
    }

    (tokens as f64 / elapsed_ms as f64 * 1000.0) as f32
}

fn http_sse_request<F>(
    host: &str,
    port: u16,
    method: &str,
    path: &str,
    body: &Value,
    timeout_ms: u64,
    cancel_token: &AtomicBool,
    mut on_value: F,
) -> Result<(), String>
where
    F: FnMut(Value) -> Result<bool, String>,
{
    let body_bytes =
        serde_json::to_vec(body).map_err(|err| format!("failed to encode request JSON: {err}"))?;
    let address = format!("{host}:{port}")
        .parse::<SocketAddr>()
        .map_err(|err| format!("invalid local engine address {host}:{port}: {err}"))?;
    let timeout = std::time::Duration::from_millis(timeout_ms);
    let mut stream = TcpStream::connect_timeout(&address, timeout)
        .map_err(|err| format!("failed to connect to local engine at {host}:{port}: {err}"))?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|err| format!("failed to set read timeout: {err}"))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|err| format!("failed to set write timeout: {err}"))?;

    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}:{port}\r\nUser-Agent: Kivarro/0.1\r\nAccept: text/event-stream\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body_bytes.len()
    );
    std::io::Write::write_all(&mut stream, request.as_bytes())
        .map_err(|err| format!("failed to write HTTP request: {err}"))?;
    std::io::Write::write_all(&mut stream, &body_bytes)
        .map_err(|err| format!("failed to write HTTP request body: {err}"))?;

    let (status_code, headers) = read_http_head(&mut stream)?;
    if status_code >= 400 {
        let body = read_remaining_http_body(&mut stream, &headers, MAX_HTTP_RESPONSE_BYTES)?;
        let message = serde_json::from_slice::<Value>(&body)
            .ok()
            .and_then(|value| {
                value
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or_else(|| String::from_utf8_lossy(&body).trim().to_string());
        return Err(format!(
            "local engine streaming request failed with HTTP {status_code}: {message}"
        ));
    }

    stream
        .set_read_timeout(Some(Duration::from_millis(STREAM_READ_POLL_MS)))
        .map_err(|err| format!("failed to set stream read poll timeout: {err}"))?;

    let mut sse_buffer = Vec::new();
    let mut bytes_seen = 0_u64;
    let read_deadline = Instant::now() + timeout;
    let mut process_chunk = |chunk: &[u8]| -> Result<bool, String> {
        if cancel_token.load(Ordering::SeqCst) {
            return Err("stream cancelled".to_string());
        }
        bytes_seen = bytes_seen.saturating_add(chunk.len() as u64);
        if bytes_seen > MAX_STREAM_RESPONSE_BYTES {
            return Err(format!(
                "local engine stream exceeds {} bytes",
                MAX_STREAM_RESPONSE_BYTES
            ));
        }

        process_sse_bytes(&mut sse_buffer, chunk, &mut on_value)
    };

    if headers
        .get("transfer-encoding")
        .map(|value| value.contains("chunked"))
        .unwrap_or(false)
    {
        read_chunked_transfer(
            &mut stream,
            MAX_STREAM_RESPONSE_BYTES,
            Some(cancel_token),
            Some(read_deadline),
            &mut process_chunk,
        )?;
    } else {
        read_stream_body(
            &mut stream,
            MAX_STREAM_RESPONSE_BYTES,
            Some(cancel_token),
            Some(read_deadline),
            &mut process_chunk,
        )?;
    }

    if !sse_buffer.is_empty() {
        process_sse_bytes(&mut sse_buffer, b"\n\n", &mut on_value)?;
    }

    Ok(())
}

fn read_http_head(stream: &mut TcpStream) -> Result<(u16, HashMap<String, String>), String> {
    let mut raw = Vec::new();
    let mut byte = [0_u8; 1];
    while raw.len() < MAX_HTTP_HEADER_BYTES {
        let count = stream
            .read(&mut byte)
            .map_err(|err| format!("failed to read HTTP response head: {err}"))?;
        if count == 0 {
            return Err("connection closed before HTTP response head completed".to_string());
        }
        raw.push(byte[0]);
        if raw.ends_with(b"\r\n\r\n") {
            return parse_http_head(&raw);
        }
    }

    Err(format!(
        "HTTP response head exceeds {} bytes",
        MAX_HTTP_HEADER_BYTES
    ))
}

fn parse_http_head(raw: &[u8]) -> Result<(u16, HashMap<String, String>), String> {
    let header_text = String::from_utf8_lossy(raw);
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

    Ok((status_code, headers))
}

fn read_remaining_http_body(
    stream: &mut TcpStream,
    headers: &HashMap<String, String>,
    max_bytes: u64,
) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    if headers
        .get("transfer-encoding")
        .map(|value| value.contains("chunked"))
        .unwrap_or(false)
    {
        read_chunked_transfer(stream, max_bytes, None, None, |chunk| {
            body.extend_from_slice(chunk);
            Ok(true)
        })?;
    } else {
        read_stream_body(stream, max_bytes, None, None, |chunk| {
            body.extend_from_slice(chunk);
            Ok(true)
        })?;
    }

    Ok(body)
}

fn read_stream_body<F>(
    stream: &mut TcpStream,
    max_bytes: u64,
    cancel_token: Option<&AtomicBool>,
    deadline: Option<Instant>,
    mut on_chunk: F,
) -> Result<(), String>
where
    F: FnMut(&[u8]) -> Result<bool, String>,
{
    let mut bytes_seen = 0_u64;
    let mut buffer = [0_u8; 8192];
    loop {
        if cancel_token
            .map(|token| token.load(Ordering::SeqCst))
            .unwrap_or(false)
        {
            return Err("stream cancelled".to_string());
        }
        let count = read_with_optional_cancel(
            stream,
            &mut buffer,
            cancel_token,
            deadline,
            "failed to read HTTP response body",
        )?;
        if count == 0 {
            return Ok(());
        }
        bytes_seen = bytes_seen.saturating_add(count as u64);
        if bytes_seen > max_bytes {
            return Err(format!("HTTP response body exceeds {max_bytes} bytes"));
        }
        if !on_chunk(&buffer[..count])? {
            return Ok(());
        }
    }
}

fn read_chunked_transfer<F>(
    stream: &mut TcpStream,
    max_bytes: u64,
    cancel_token: Option<&AtomicBool>,
    deadline: Option<Instant>,
    mut on_chunk: F,
) -> Result<(), String>
where
    F: FnMut(&[u8]) -> Result<bool, String>,
{
    let mut bytes_seen = 0_u64;
    loop {
        if cancel_token
            .map(|token| token.load(Ordering::SeqCst))
            .unwrap_or(false)
        {
            return Err("stream cancelled".to_string());
        }
        let Some(size_line) = read_http_line(stream, cancel_token, deadline)? else {
            return Err("chunked response ended before chunk size".to_string());
        };
        let size_text = String::from_utf8_lossy(&size_line);
        let size_hex = size_text.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_hex, 16)
            .map_err(|err| format!("invalid chunk size {size_hex}: {err}"))?;
        if size == 0 {
            consume_trailing_chunk_headers(stream, cancel_token, deadline)?;
            return Ok(());
        }

        bytes_seen = bytes_seen.saturating_add(size as u64);
        if bytes_seen > max_bytes {
            return Err(format!("chunked response exceeds {max_bytes} bytes"));
        }

        let mut chunk = vec![0_u8; size];
        read_exact_with_optional_cancel(
            stream,
            &mut chunk,
            cancel_token,
            deadline,
            "failed to read chunk data",
        )?;
        let mut crlf = [0_u8; 2];
        read_exact_with_optional_cancel(
            stream,
            &mut crlf,
            cancel_token,
            deadline,
            "failed to read chunk terminator",
        )?;
        if crlf != *b"\r\n" {
            return Err("chunked response contained an invalid chunk terminator".to_string());
        }

        if !on_chunk(&chunk)? {
            return Ok(());
        }
    }
}

fn read_http_line(
    stream: &mut TcpStream,
    cancel_token: Option<&AtomicBool>,
    deadline: Option<Instant>,
) -> Result<Option<Vec<u8>>, String> {
    let mut line = Vec::new();
    let mut byte = [0_u8; 1];
    loop {
        let count = read_with_optional_cancel(
            stream,
            &mut byte,
            cancel_token,
            deadline,
            "failed to read HTTP line",
        )?;
        if count == 0 {
            return if line.is_empty() {
                Ok(None)
            } else {
                Err("connection closed in the middle of an HTTP line".to_string())
            };
        }

        line.push(byte[0]);
        if line.len() > MAX_HTTP_HEADER_BYTES {
            return Err(format!("HTTP line exceeds {} bytes", MAX_HTTP_HEADER_BYTES));
        }
        if byte[0] == b'\n' {
            while matches!(line.last(), Some(b'\r' | b'\n')) {
                line.pop();
            }
            return Ok(Some(line));
        }
    }
}

fn consume_trailing_chunk_headers(
    stream: &mut TcpStream,
    cancel_token: Option<&AtomicBool>,
    deadline: Option<Instant>,
) -> Result<(), String> {
    while let Some(line) = read_http_line(stream, cancel_token, deadline)? {
        if line.is_empty() {
            return Ok(());
        }
    }

    Ok(())
}

fn read_exact_with_optional_cancel(
    stream: &mut TcpStream,
    buffer: &mut [u8],
    cancel_token: Option<&AtomicBool>,
    deadline: Option<Instant>,
    context: &str,
) -> Result<(), String> {
    let mut offset = 0;
    while offset < buffer.len() {
        let count = read_with_optional_cancel(
            stream,
            &mut buffer[offset..],
            cancel_token,
            deadline,
            context,
        )?;
        if count == 0 {
            return Err(format!(
                "{context}: connection closed before expected bytes"
            ));
        }
        offset += count;
    }

    Ok(())
}

fn read_with_optional_cancel(
    stream: &mut TcpStream,
    buffer: &mut [u8],
    cancel_token: Option<&AtomicBool>,
    deadline: Option<Instant>,
    context: &str,
) -> Result<usize, String> {
    loop {
        if cancel_token
            .map(|token| token.load(Ordering::SeqCst))
            .unwrap_or(false)
        {
            return Err("stream cancelled".to_string());
        }
        if deadline
            .map(|deadline| Instant::now() >= deadline)
            .unwrap_or(false)
        {
            return Err(format!("{context}: read timed out"));
        }

        match stream.read(buffer) {
            Ok(count) => return Ok(count),
            Err(err)
                if cancel_token.is_some()
                    && matches!(err.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) =>
            {
                continue;
            }
            Err(err) => return Err(format!("{context}: {err}")),
        }
    }
}

fn process_sse_bytes<F>(
    buffer: &mut Vec<u8>,
    chunk: &[u8],
    on_value: &mut F,
) -> Result<bool, String>
where
    F: FnMut(Value) -> Result<bool, String>,
{
    buffer.extend_from_slice(chunk);
    while let Some((boundary_start, boundary_len)) = find_sse_boundary(buffer) {
        let frame = buffer[..boundary_start].to_vec();
        buffer.drain(..boundary_start + boundary_len);
        if !process_sse_frame(&frame, on_value)? {
            return Ok(false);
        }
    }

    Ok(true)
}

fn find_sse_boundary(buffer: &[u8]) -> Option<(usize, usize)> {
    let lf = buffer
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|position| (position, 2));
    let crlf = buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|position| (position, 4));

    match (lf, crlf) {
        (Some(left), Some(right)) => Some(if left.0 <= right.0 { left } else { right }),
        (Some(boundary), None) | (None, Some(boundary)) => Some(boundary),
        (None, None) => None,
    }
}

fn process_sse_frame<F>(frame: &[u8], on_value: &mut F) -> Result<bool, String>
where
    F: FnMut(Value) -> Result<bool, String>,
{
    let text = String::from_utf8_lossy(frame);
    let data = text
        .lines()
        .filter_map(|line| {
            let line = line.trim_end_matches('\r');
            line.strip_prefix("data:")
                .map(|value| value.trim_start().to_string())
        })
        .collect::<Vec<_>>();
    if data.is_empty() {
        return Ok(true);
    }

    let payload = data.join("\n");
    if payload.trim() == "[DONE]" {
        return Ok(false);
    }

    let value = serde_json::from_str::<Value>(&payload)
        .map_err(|err| format!("failed to parse SSE JSON payload: {err}; payload: {payload}"))?;
    on_value(value)
}

fn extract_stream_chunk(value: &Value) -> StreamChunk {
    let delta = value
        .pointer("/choices/0/delta/content")
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .pointer("/choices/0/message/content")
                .and_then(Value::as_str)
        })
        .or_else(|| value.pointer("/choices/0/text").and_then(Value::as_str))
        .or_else(|| value.pointer("/content").and_then(Value::as_str))
        .unwrap_or_default()
        .to_string();
    let finish_reason = value
        .pointer("/choices/0/finish_reason")
        .and_then(Value::as_str)
        .or_else(|| value.pointer("/stop_type").and_then(Value::as_str))
        .filter(|reason| !reason.is_empty() && *reason != "none")
        .map(str::to_string);
    let stop = value
        .pointer("/stop")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || finish_reason.is_some();

    StreamChunk {
        delta,
        finish_reason,
        stop,
    }
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
    Ok(vec![model_library_directory()?])
}

fn model_library_directory() -> Result<PathBuf, String> {
    let current_dir =
        env::current_dir().map_err(|err| format!("failed to read current directory: {err}"))?;
    Ok(current_dir.join("models"))
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

        models.push(model_record_from_path(&path)?);
    }

    Ok(())
}

fn model_record_from_path(path: &Path) -> Result<ModelRecord, String> {
    let metadata = fs::metadata(path)
        .map_err(|err| format!("failed to read model metadata {}: {err}", path.display()))?;
    let fallback_name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("Unnamed model")
        .to_string();
    let gguf_metadata = read_model_gguf_metadata(path);
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

    Ok(ModelRecord {
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
    })
}

fn unique_model_destination(directory: &Path, file_name: &str) -> PathBuf {
    let candidate = directory.join(file_name);
    if !candidate.exists() {
        return candidate;
    }

    let path = Path::new(file_name);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("model");
    let extension = path.extension().and_then(|value| value.to_str());

    for suffix in 2..=9999 {
        let next_name = match extension {
            Some(extension) if !extension.is_empty() => format!("{stem}-{suffix}.{extension}"),
            _ => format!("{stem}-{suffix}"),
        };
        let next = directory.join(next_name);
        if !next.exists() {
            return next;
        }
    }

    directory.join(format!("{stem}-{}", unix_timestamp()))
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
    use std::net::TcpListener;

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

    #[test]
    fn builds_mistralrs_serve_args_from_model_path() {
        let args =
            build_mistralrs_serve_args(Path::new("D:/models/mistral.gguf"), DEFAULT_API_HOST, 9091);

        assert_eq!(args.first().map(String::as_str), Some("serve"));
        assert_arg_pair(&args, "-m", "D:/models/mistral.gguf");
        assert_arg_pair(&args, "--host", DEFAULT_API_HOST);
        assert_arg_pair(&args, "-p", "9091");
        assert!(args.iter().any(|arg| arg == "--no-ui"));
        assert_eq!(canonical_backend("mistralrs"), Some(BACKEND_MISTRAL_RS));
        assert_eq!(
            request_model_name(BACKEND_MISTRAL_RS, "Mistral Local"),
            "default"
        );
    }

    #[test]
    fn normalizes_api_settings_and_base_url() {
        let settings = normalize_api_settings(ApiSettings {
            host: "  localhost ".to_string(),
            port: 9099,
        })
        .expect("valid API settings");

        assert_eq!(settings.host, "localhost");
        assert_eq!(settings.port, 9099);
        assert_eq!(
            api_base_url(&settings.host, settings.port),
            "http://localhost:9099/v1"
        );
    }

    #[test]
    fn rejects_invalid_api_settings() {
        assert!(normalize_api_settings(ApiSettings {
            host: "local host".to_string(),
            port: 8080,
        })
        .is_err());
        assert!(normalize_api_settings(ApiSettings {
            host: "127.0.0.1".to_string(),
            port: 0,
        })
        .is_err());
    }

    #[test]
    fn creates_unique_model_destination_names() {
        let directory = env::temp_dir().join(format!(
            "kivarro-model-destination-{}-{}",
            std::process::id(),
            unix_timestamp()
        ));
        fs::create_dir_all(&directory).expect("create model destination test directory");
        File::create(directory.join("sample.gguf")).expect("create first model");
        File::create(directory.join("sample-2.gguf")).expect("create second model");

        let destination = unique_model_destination(&directory, "sample.gguf");

        assert_eq!(
            destination.file_name().and_then(|name| name.to_str()),
            Some("sample-3.gguf")
        );

        fs::remove_dir_all(directory).expect("remove model destination test directory");
    }

    #[test]
    fn parses_nvidia_smi_gpu_telemetry() {
        let blocks = parse_nvidia_smi_gpus("NVIDIA RTX 4090, 57, 24564, 12000\n");

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].name, "NVIDIA RTX 4090");
        assert_eq!(blocks[0].kind, "GPU");
        assert_eq!(blocks[0].utilization_percent, 57.0);
        assert_eq!(blocks[0].memory_total_gib, Some(24.0));
        assert_eq!(blocks[0].memory_used_gib, Some(11.7));
    }

    #[test]
    fn parses_quoted_csv_fields() {
        let fields = parse_csv_line("\"Name, With Comma\",\"4294967296\"");

        assert_eq!(fields, vec!["Name, With Comma", "4294967296"]);
    }

    #[test]
    fn chunks_and_ranks_knowledge_retrieval() {
        let chunks = chunk_document_text(
            "GPU offload moves transformer layers into VRAM.\n\nContext windows increase KV cache pressure.",
        );
        assert_eq!(chunks.len(), 1);

        let store = KnowledgeStore {
            bases: vec![KnowledgeBase {
                id: "research-vault".to_string(),
                name: "Research Vault".to_string(),
                document_count: 1,
                chunk_count: 1,
                updated_at: "test".to_string(),
            }],
            documents: Vec::new(),
            chunks: vec![KnowledgeChunk {
                id: "chunk-1".to_string(),
                knowledge_base_id: "research-vault".to_string(),
                document_id: "doc-1".to_string(),
                document_name: "notes.md".to_string(),
                chunk_index: 0,
                content: chunks[0].clone(),
            }],
        };
        let query_terms = tokenize_for_retrieval("VRAM context cache");
        let matches = rank_retrieval_matches(&store, "research-vault", &query_terms);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].document_name, "notes.md");
        assert!(matches[0].score > 0.6);
        assert!(matches[0].snippet.contains("VRAM"));
    }

    #[test]
    fn parses_split_openai_sse_chat_deltas() {
        let mut buffer = Vec::new();
        let mut deltas = Vec::new();
        let mut keep_going = process_sse_bytes(
            &mut buffer,
            br#"data: {"choices":[{"delta":{"content":"hel"},"finish_reason":null}]}"#,
            &mut |value| {
                deltas.push(extract_stream_chunk(&value));
                Ok(true)
            },
        )
        .expect("first split SSE chunk");
        assert!(keep_going);
        assert!(deltas.is_empty());

        keep_going = process_sse_bytes(
            &mut buffer,
            b"\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"lo\"},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n",
            &mut |value| {
                deltas.push(extract_stream_chunk(&value));
                Ok(true)
            },
        )
        .expect("second split SSE chunk");

        assert!(!keep_going);
        assert_eq!(deltas.len(), 2);
        assert_eq!(deltas[0].delta, "hel");
        assert!(!deltas[0].stop);
        assert_eq!(deltas[1].delta, "lo");
        assert_eq!(deltas[1].finish_reason.as_deref(), Some("stop"));
        assert!(deltas[1].stop);
    }

    #[test]
    fn extracts_llama_completion_stream_content() {
        let value = json!({
            "content": " token",
            "tokens": [123],
            "stop": false
        });
        let chunk = extract_stream_chunk(&value);

        assert_eq!(chunk.delta, " token");
        assert!(!chunk.stop);
        assert!(chunk.finish_reason.is_none());
    }

    #[test]
    fn cancels_idle_sse_stream_promptly() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind SSE test listener");
        let port = listener
            .local_addr()
            .expect("read SSE test listener address")
            .port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept SSE test connection");
            let mut request = [0_u8; 512];
            let _ = stream.read(&mut request);
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n",
                )
                .expect("write SSE response head");
            std::thread::sleep(Duration::from_millis(1_500));
        });

        let cancel_token = Arc::new(AtomicBool::new(false));
        let canceller = Arc::clone(&cancel_token);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(250));
            canceller.store(true, Ordering::SeqCst);
        });

        let started = Instant::now();
        let result = http_sse_request(
            "127.0.0.1",
            port,
            "POST",
            "/v1/chat/completions",
            &json!({ "stream": true }),
            10_000,
            &cancel_token,
            |_| Ok(true),
        );

        assert!(matches!(result, Err(ref err) if err == "stream cancelled"));
        assert!(started.elapsed() < Duration::from_millis(1_000));
        server.join().expect("join SSE test server");
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
            import_model_file,
            list_inference_profiles,
            save_inference_profile,
            delete_inference_profile,
            get_model_load_plan,
            get_engine_status,
            start_inference_engine,
            start_llama_server,
            stop_inference_engine,
            stop_llama_server,
            cancel_chat_completion_stream,
            run_chat_completion,
            run_chat_completion_stream,
            get_api_settings,
            save_api_settings,
            get_api_status,
            run_benchmark,
            list_benchmark_results,
            list_knowledge_bases,
            create_knowledge_base,
            list_knowledge_documents,
            import_knowledge_document,
            test_knowledge_retrieval,
            list_system_logs
        ])
        .run(tauri::generate_context!())
        .expect("error while running Kivarro");
}
