use serde::Serialize;
use std::{
    env, fs,
    path::{Path, PathBuf},
};
use sysinfo::System;

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

#[derive(Debug, Serialize)]
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
            get_api_status,
            list_benchmark_results,
            list_system_logs
        ])
        .run(tauri::generate_context!())
        .expect("error while running Kivarro");
}
