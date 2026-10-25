use crate::config::SimConfig;
use crate::snapshot::{SnapshotBuilder, WorldSnapshot};
use crate::telemetry::{SimEvent, Telemetry};
use crate::world::{HistoryPoint, World, WorldStats};
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
    pub snapshot_tx: broadcast::Sender<WorldSnapshot>,
}

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(index_handler))
        .route("/ws", get(ws_handler))
        .route("/api/stats", get(stats_handler))
        .route("/api/history", get(history_handler))
        .route("/api/events", get(events_handler))
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
            // keep alive ping/pong
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

async fn history_handler(State(state): State<Arc<AppState>>) -> Json<Vec<HistoryPoint>> {
    let w = state.world.read();
    Json(w.history.clone())
}

async fn events_handler(State(state): State<Arc<AppState>>) -> Json<Vec<SimEvent>> {
    Json(state.telemetry.get_recent_events())
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
    let mut w = state.world.write();
    *w = World::new(SimConfig::default());
    state.telemetry.log(0, "INFO", "Simulation world reset to baseline state.".to_string());
    Json("SIMULATION_RESET")
}
