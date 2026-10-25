mod brain;
mod chemical;
mod cli;
mod config;
mod creature;
mod engine;
mod genetics;
mod math;
mod server;
mod snapshot;
mod spatial;
mod telemetry;
mod world;

use cli::CliArgs;
use config::SimConfig;
use engine::SimEngine;
use parking_lot::RwLock;
use server::{create_router, AppState};
use std::sync::Arc;
use telemetry::Telemetry;
use tokio::sync::broadcast;
use world::World;

#[tokio::main]
async fn main() {
    let args = CliArgs::parse();
    let telemetry = Telemetry::new();

    Telemetry::print_banner(&args.host, args.port, args.headless);

    let mut config = SimConfig::default();
    config.initial_creatures = args.initial_creatures;
    config.initial_food = args.initial_food;
    config.world_width = args.world_size;
    config.world_height = args.world_size;
    config.time_scale = args.speed;

    let world = Arc::new(RwLock::new(World::new(config)));
    let (snapshot_tx, _) = broadcast::channel(32);

    let engine = SimEngine::new(world.clone(), telemetry.clone(), snapshot_tx.clone());

    if args.headless {
        engine.run_benchmark(args.benchmark_ticks);
        return;
    }

    // Spawn simulation tick loop
    tokio::spawn(async move {
        engine.run_loop().await;
    });

    // Start HTTP & WebSocket server
    let app_state = Arc::new(AppState {
        world,
        telemetry,
        snapshot_tx,
    });

    let router = create_router(app_state);
    let bind_addr = format!("{}:{}", args.host, args.port);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind to {}: {}", bind_addr, e));

    println!(">>> Engine running! Connect in browser: http://{}", bind_addr);

    axum::serve(listener, router).await.expect("Server failed");
}
