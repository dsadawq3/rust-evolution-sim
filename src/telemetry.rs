use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimEvent {
    pub tick: u64,
    pub level: String, // "INFO", "WARN", "COMBAT", "MUTATION"
    pub message: String,
}

#[derive(Clone)]
pub struct Telemetry {
    events: Arc<RwLock<VecDeque<SimEvent>>>,
    last_tps_check: Arc<RwLock<(Instant, u64, f32)>>,
}

impl Telemetry {
    pub fn new() -> Self {
        Telemetry {
            events: Arc::new(RwLock::new(VecDeque::with_capacity(120))),
            last_tps_check: Arc::new(RwLock::new((Instant::now(), 0, 0.0))),
        }
    }

    pub fn log(&self, tick: u64, level: &str, message: String) {
        let event = SimEvent {
            tick,
            level: level.to_string(),
            message,
        };

        let mut queue = self.events.write();
        if queue.len() >= 100 {
            queue.pop_front();
        }
        queue.push_back(event);
    }

    pub fn get_recent_events(&self) -> Vec<SimEvent> {
        let queue = self.events.read();
        queue.iter().cloned().collect()
    }

    pub fn update_tps(&self, current_tick: u64) -> f32 {
        let mut check = self.last_tps_check.write();
        let elapsed = check.0.elapsed().as_secs_f32();
        if elapsed >= 0.5 {
            let ticks_done = current_tick - check.1;
            let current_tps = ticks_done as f32 / elapsed;
            check.0 = Instant::now();
            check.1 = current_tick;
            check.2 = current_tps;
            current_tps
        } else {
            check.2
        }
    }

    pub fn print_banner(host: &str, port: u16, headless: bool) {
        println!("┌─────────────────────────────────────────────────────────────┐");
        println!("│           AI EVOLUTION SIMULATOR (RUST ENGINE 2.0)          │");
        println!("│           Diploid Genetics • CTRNN Brain • Pheromones       │");
        println!("└─────────────────────────────────────────────────────────────┘");
        if !headless {
            println!("  [WEB UI] Available at: http://{}:{}", host, port);
            println!("  [STATS]  Streaming snapshots via WebSocket: ws://{}:{}/ws", host, port);
        } else {
            println!("  [HEADLESS] Running benchmark mode...");
        }
        println!();
    }
}
