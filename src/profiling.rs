use actix_web::{get, HttpResponse, Result as ActixResult};
use pprof::ProfilerGuard;
use std::sync::Mutex;
use thiserror::Error;
use tracing::{error, info};

#[get("/debug/pprof/profile")]
pub async fn get_profile() -> ActixResult<HttpResponse> {
    match get_flamegraph_internal() {
        Ok(flamegraph) => Ok(HttpResponse::Ok()
            .content_type("image/svg+xml")
            .body(flamegraph)),
        Err(e) => {
            error!("Failed to generate flamegraph: {}", e);
            Ok(HttpResponse::InternalServerError()
                .body(format!("Failed to generate profile: {}", e)))
        }
    }
}

#[derive(Clone)]
pub struct ProfilingConfig {
    pub enabled: bool,
}

impl Default for ProfilingConfig {
    fn default() -> Self {
        Self {
            enabled: std::env::var("ENABLE_PROFILING")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(false),
        }
    }
}

#[derive(Error, Debug)]
pub enum ProfilingError {
    #[error("Failed to create profiler guard: {0}")]
    GuardCreation(String),
    #[error("Failed to build profiler report: {0}")]
    ReportBuild(String),
    #[error("Failed to generate flamegraph: {0}")]
    FlamegraphGeneration(String),
}

// Global profiler state
static PROFILER: Mutex<Option<ProfilerGuard<'static>>> = Mutex::new(None);

pub fn initialize(config: &ProfilingConfig) -> Result<(), ProfilingError> {
    if config.enabled {
        let guard =
            ProfilerGuard::new(100).map_err(|e| ProfilingError::GuardCreation(e.to_string()))?;

        match PROFILER.lock() {
            Ok(mut profiler) => {
                *profiler = Some(guard);
                info!("CPU profiling enabled");
                Ok(())
            }
            Err(e) => {
                error!("Failed to acquire profiler lock: {}", e);
                Err(ProfilingError::GuardCreation(
                    "Failed to acquire profiler lock".to_string(),
                ))
            }
        }
    } else {
        Ok(())
    }
}

fn get_flamegraph_internal() -> Result<Vec<u8>, ProfilingError> {
    let profiler = PROFILER.lock().map_err(|e| {
        ProfilingError::GuardCreation(format!("Failed to acquire profiler lock: {}", e))
    })?;

    let guard = profiler
        .as_ref()
        .ok_or_else(|| ProfilingError::GuardCreation("Profiler not initialized".to_string()))?;

    let report = guard
        .report()
        .build()
        .map_err(|e| ProfilingError::ReportBuild(e.to_string()))?;

    let mut flamegraph = Vec::new();
    report
        .flamegraph(&mut flamegraph)
        .map_err(|e| ProfilingError::FlamegraphGeneration(e.to_string()))?;

    Ok(flamegraph)
}
