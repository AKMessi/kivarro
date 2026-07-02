use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use sysinfo::System;
use tauri::{AppHandle, Manager};

const PROFILE_EXTENSION: &str = "kivarro.json";

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
fn get_runtime_metrics() -> Result<RuntimeMetrics, String> {
    let snapshot = get_hardware_snapshot()?;

    Ok(RuntimeMetrics {
        active_model: "No model loaded".to_string(),
        active_backend: "Engine idle".to_string(),
        server_url: "http://127.0.0.1:8080/v1".to_string(),
        api_port: 8080,
        api_online: false,
        tokens_per_second: 0.0,
        context_used_tokens: 0,
        context_total_tokens: 32768,
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
    let model_name = model_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("model");
    let estimated_layers = estimate_layer_count(model_name);
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

    let recommendation = if fit == "Fits available RAM" && gpu_layers == estimated_layers {
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
fn get_api_status() -> Result<ApiStatus, String> {
    Ok(ApiStatus {
        enabled: false,
        port: 8080,
        base_url: "http://127.0.0.1:8080/v1".to_string(),
        endpoints: vec![
            ApiEndpoint {
                method: "POST".to_string(),
                path: "/v1/chat/completions".to_string(),
                description: "OpenAI-compatible streaming chat completions".to_string(),
                status: "Planned".to_string(),
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
                status: "Planned".to_string(),
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

fn estimate_layer_count(model_name: &str) -> u16 {
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
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("Unnamed model")
            .to_string();
        let format = path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("unknown")
            .to_ascii_uppercase();
        let size_gib = bytes_to_gib(metadata.len());

        models.push(ModelRecord {
            id: path.to_string_lossy().to_string(),
            name,
            path: path.to_string_lossy().to_string(),
            format,
            size_gib,
            status: "Discovered".to_string(),
            fit: model_fit_label(size_gib),
        });
    }

    Ok(())
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_hardware_snapshot,
            get_runtime_metrics,
            list_models,
            list_inference_profiles,
            save_inference_profile,
            delete_inference_profile,
            get_model_load_plan,
            get_api_status,
            list_benchmark_results,
            list_system_logs
        ])
        .run(tauri::generate_context!())
        .expect("error while running Kivarro");
}
