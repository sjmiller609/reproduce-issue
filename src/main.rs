use actix_web::{get, App, HttpResponse, HttpServer};
use apalis::prelude::*;
use apalis_sql::postgres::PostgresStorage;
use futures::future;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{error, info};
mod profiling;

#[derive(Debug, Serialize, Deserialize)]
struct Job1 {
    id: i32,
}

#[derive(Debug, Serialize, Deserialize)]
struct Job2 {
    name: String,
}

async fn job1_handler(_job: Job1, task_id: TaskId) -> Result<(), Error> {
    info!("Processing Job1: {}", task_id);
    Ok(())
}

async fn job2_handler(_job: Job2, task_id: TaskId) -> Result<(), Error> {
    info!("Processing Job2: {}", task_id);
    Ok(())
}

#[get("/health")]
async fn health_check() -> HttpResponse {
    HttpResponse::Ok().finish()
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt().init();
    let profiling_config = profiling::ProfilingConfig::default();
    profiling::initialize(&profiling_config).expect("Failed to initialize profiling");

    let addr = "0.0.0.0:8080";
    println!("Starting web server at http://{}", addr);

    let web_server = HttpServer::new(move || {
        App::new()
            .service(health_check)
            .service(profiling::get_profile)
    })
    .bind(addr)?
    .run();

    println!("Starting job server");

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    PostgresStorage::setup(&pool)
        .await
        .expect("unable to run migrations for postgres");

    let job1_storage: PostgresStorage<Job1> = PostgresStorage::new(pool.clone());
    let job2_storage: PostgresStorage<Job2> = PostgresStorage::new(pool.clone());

    let job_server = Monitor::new()
        .register({
            WorkerBuilder::new("job1")
                .backend(job1_storage)
                .build_fn(job1_handler)
        })
        .register({
            WorkerBuilder::new("job2")
                .backend(job2_storage)
                .build_fn(job2_handler)
        })
        .on_event(|e| info!("{e}"))
        .shutdown_timeout(std::time::Duration::from_secs(5))
        .run_with_signal(tokio::signal::ctrl_c());

    let web_future = web_server;
    let job_future = job_server;

    match future::try_join(web_future, job_future).await {
        Ok(_) => {
            info!("Servers shut down gracefully");
            Ok(())
        }
        Err(e) => {
            error!("Server error: {}", e);
            Err(e)
        }
    }
}
