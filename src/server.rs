use crate::config::SimConfig;
use crate::snapshot::{SnapshotBuilder, WorldSnapshot};
use crate::species::{SpeciesAnalyzer, SpeciesInfo};
use crate::storage::{DetailedHistoryEntry, SimulationMeta, StorageManager};
use crate::telemetry::{SimEvent, Telemetry};
use crate::world::{World, WorldStats};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use futures::{sink::SinkExt, stream::StreamExt};
use parking_lot::RwLock;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

pub struct AppState {
    pub world: Arc<RwLock<World>>,
    pub telemetry: Telemetry,
    pub storage: Arc<RwLock<StorageManager>>,
    pub snapshot_tx: broadcast::Sender<WorldSnapshot>,
}

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(index_handler))
        .route("/ws", get(ws_handler))
        .route("/api/stats", get(stats_handler))
        .route("/api/history", get(history_handler))
        .route("/api/timeline", get(timeline_handler))
        .route("/api/species", get(species_handler))
        .route("/api/events", get(events_handler))
        .route("/api/simulations", get(simulations_list_handler))
        .route("/api/simulations/new", post(new_simulation_handler))
        .route("/api/simulations/switch/:name", post(switch_simulation_handler))
        .route("/api/config/current", get(current_config_handler))
        .route("/api/config/update", post(config_update_handler))
        .route("/api/creature/:id", get(creature_detail_handler))
        .route("/api/speed", post(speed_handler))
        .route("/api/action/meteor", post(meteor_handler))
        .route("/api/action/radiation", post(radiation_handler))
        .route("/api/action/food", post(food_handler))
        .route("/api/action/meat", post(meat_handler))
        .route("/api/action/reset", post(reset_handler))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn index_handler() -> impl IntoResponse {
    (
        [
            ("Content-Type", "text/html; charset=utf-8"),
            ("Cache-Control", "no-cache, no-store, must-revalidate"),
            ("Pragma", "no-cache"),
            ("Expires", "0"),
        ],
        include_str!("web/index.html"),
    )
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.snapshot_tx.subscribe();

    let mut send_task = tokio::spawn(async move {
        while let Ok(snapshot) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&snapshot) {
                if sender.send(Message::Text(json)).await.is_err() {
                    break;
                }
            }
        }
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(_msg)) = receiver.next().await {
            // ping/pong
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };
}

async fn stats_handler(State(state): State<Arc<AppState>>) -> Json<WorldStats> {
    let w = state.world.read();
    Json(w.get_stats())
}

async fn history_handler(State(state): State<Arc<AppState>>) -> Json<Vec<DetailedHistoryEntry>> {
    let storage = state.storage.read();
    let active_name = storage.get_active_name();
    let recent = storage.read_recent_history(&active_name, 100);
    Json(recent)
}

async fn timeline_handler(State(state): State<Arc<AppState>>) -> Json<Option<crate::storage::ChartTimelineData>> {
    let storage = state.storage.read();
    let active_name = storage.get_active_name();
    Json(storage.get_timeline(&active_name))
}

async fn current_config_handler(State(state): State<Arc<AppState>>) -> Json<SimConfig> {
    let w = state.world.read();
    Json(w.config.clone())
}

async fn species_handler(State(state): State<Arc<AppState>>) -> Json<Vec<SpeciesInfo>> {
    let w = state.world.read();
    Json(SpeciesAnalyzer::classify_population(&w.creatures))
}

async fn events_handler(State(state): State<Arc<AppState>>) -> Json<Vec<SimEvent>> {
    Json(state.telemetry.get_recent_events())
}

async fn simulations_list_handler(State(state): State<Arc<AppState>>) -> Json<Vec<SimulationMeta>> {
    let storage = state.storage.read();
    Json(storage.list_simulations())
}

#[derive(Deserialize)]
pub struct NewSimPayload {
    pub name: String,
    pub creatures: Option<usize>,
    pub food: Option<usize>,
    pub world_size: Option<f32>,
    pub mutation_rate: Option<f32>,
    pub enable_pioneer_reseeding: Option<bool>,
}

async fn new_simulation_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<NewSimPayload>,
) -> Json<String> {
    let mut config = SimConfig::default();
    if let Some(c) = payload.creatures { config.initial_creatures = c; }
    if let Some(f) = payload.food { config.initial_food = f; }
    if let Some(w) = payload.world_size {
        config.world_width = w;
        config.world_height = w;
    }
    if let Some(m) = payload.mutation_rate { config.mutation_rate_multiplier = m; }
    if let Some(r) = payload.enable_pioneer_reseeding { config.enable_pioneer_reseeding = r; }

    let safe_name = payload.name.trim().replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '-', "_");
    let safe_name = if safe_name.is_empty() { "sim_custom".to_string() } else { safe_name };

    {
        let mut storage = state.storage.write();
        storage.set_active_name(&safe_name);
        storage.save_config(&safe_name, &config);
    }

    {
        let mut w = state.world.write();
        *w = World::new(config);
    }

    state.telemetry.log(0, "INFO", format!("Создан и запущен новый мир '{}'", safe_name));
    Json(safe_name)
}

async fn switch_simulation_handler(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Json<&'static str> {
    let mut storage = state.storage.write();
    storage.set_active_name(&name);

    if let Some(config) = storage.load_config(&name) {
        let mut w = state.world.write();
        *w = World::new(config);
    } else {
        let mut w = state.world.write();
        *w = World::new(SimConfig::default());
    }

    state.telemetry.log(0, "INFO", format!("Switched to simulation '{}'", name));
    Json("SWITCHED")
}

#[derive(Deserialize)]
pub struct ConfigUpdatePayload {
    pub mutation_rate: Option<f32>,
    pub food_spawn_rate: Option<f32>,
    pub base_metabolism: Option<f32>,
    pub enable_pioneer_reseeding: Option<bool>,
}

async fn config_update_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ConfigUpdatePayload>,
) -> Json<&'static str> {
    let mut w = state.world.write();
    if let Some(m) = payload.mutation_rate {
        w.config.mutation_rate_multiplier = m.clamp(0.05, 20.0);
    }
    if let Some(f) = payload.food_spawn_rate {
        w.config.food_spawn_per_sec = f.clamp(0.0, 200.0);
    }
    if let Some(b) = payload.base_metabolism {
        w.config.base_metabolism = b.clamp(0.005, 0.5);
    }
    if let Some(r) = payload.enable_pioneer_reseeding {
        w.config.enable_pioneer_reseeding = r;
    }
    state.telemetry.log(w.tick, "CONFIG", "Параметры мира обновлены".to_string());
    Json("CONFIG_UPDATED")
}

async fn creature_detail_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<usize>,
) -> impl IntoResponse {
    let w = state.world.read();
    if let Some(c) = w.creatures.iter().find(|c| c.id == id && c.is_alive) {
        Json(Some(SnapshotBuilder::build_creature_detail(c)))
    } else {
        Json(None)
    }
}

#[derive(Deserialize)]
pub struct SpeedPayload {
    pub speed: f32,
    pub paused: Option<bool>,
}

async fn speed_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SpeedPayload>,
) -> Json<&'static str> {
    let mut w = state.world.write();
    w.config.time_scale = payload.speed.clamp(0.0, 100.0);
    if let Some(p) = payload.paused {
        w.config.is_paused = p;
    }
    Json("OK")
}

async fn meteor_handler(State(state): State<Arc<AppState>>) -> Json<&'static str> {
    let mut w = state.world.write();
    w.disaster_meteor();
    state.telemetry.log(w.tick, "WARN", "METEOR EXTINCTION STRIKE: 75% population annihilated!".to_string());
    Json("METEOR_STRIKE_APPLIED")
}

async fn radiation_handler(State(state): State<Arc<AppState>>) -> Json<&'static str> {
    let mut w = state.world.write();
    w.radiation_burst();
    state.telemetry.log(w.tick, "MUTATION", "COSMIC RADIATION BURST: Mutation rate multiplied 8x!".to_string());
    Json("RADIATION_BURST_APPLIED")
}

async fn food_handler(State(state): State<Arc<AppState>>) -> Json<&'static str> {
    let mut w = state.world.write();
    w.spawn_food_cluster(70);
    Json("FOOD_SPAWNED")
}

async fn meat_handler(State(state): State<Arc<AppState>>) -> Json<&'static str> {
    let mut w = state.world.write();
    w.spawn_meat_bomb(35);
    Json("MEAT_BOMB_SPAWNED")
}

async fn reset_handler(State(state): State<Arc<AppState>>) -> Json<&'static str> {
    let active_name = {
        let storage = state.storage.read();
        storage.get_active_name()
    };

    let (stats, max_gen, dominant_name, loaded_cfg) = {
        let w = state.world.read();
        let stats = w.get_stats();
        let max_gen = stats.max_generation;
        let spec = SpeciesAnalyzer::classify_population(&w.creatures);
        let dom = spec.first().map(|s| s.name.clone()).unwrap_or_else(|| "None".to_string());
        (stats, max_gen, dom, w.config.clone())
    };

    {
        let storage = state.storage.write();
        storage.archive_and_reset(&active_name, &stats, max_gen, &dominant_name);
    }

    {
        let mut w = state.world.write();
        *w = World::new(loaded_cfg);
    }

    state.telemetry.log(0, "INFO", format!("Симуляция '{}' заархивирована в summary.json и перезапущена с нуля.", active_name));
    Json("SIMULATION_ARCHIVED_AND_RESET")
}
