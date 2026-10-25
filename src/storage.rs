#![allow(dead_code)]
use crate::config::SimConfig;
use crate::species::SpeciesInfo;
use crate::telemetry::SimEvent;
use crate::world::HistoryPoint;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimulationMeta {
    pub name: String,
    pub created_at: u64,
    pub last_tick: u64,
    pub total_births: u64,
    pub total_deaths: u64,
    pub peak_population: usize,
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DetailedHistoryEntry {
    pub tick: u64,
    pub population: usize,
    pub food_count: usize,
    pub meat_count: usize,
    pub avg_eyes: f32,
    pub avg_jaw: f32,
    pub avg_radius: f32,
    pub herbivores: usize,
    pub carnivores: usize,
    pub max_generation: usize,
    pub species: Vec<SpeciesInfo>,
}

#[derive(Clone)]
pub struct StorageManager {
    base_dir: PathBuf,
    active_sim: String,
}

impl StorageManager {
    pub fn new(active_sim_name: &str) -> Self {
        let base_dir = PathBuf::from("simulations");
        let mgr = StorageManager {
            base_dir,
            active_sim: active_sim_name.to_string(),
        };
        mgr.ensure_sim_dir(active_sim_name);
        mgr
    }

    fn sim_path(&self, name: &str) -> PathBuf {
        self.base_dir.join(name)
    }

    pub fn get_active_name(&self) -> String {
        self.active_sim.clone()
    }

    pub fn set_active_name(&mut self, name: &str) {
        self.active_sim = name.to_string();
        self.ensure_sim_dir(name);
    }

    pub fn ensure_sim_dir(&self, name: &str) {
        let dir = self.sim_path(name);
        if !dir.exists() {
            let _ = fs::create_dir_all(&dir);
            let meta = SimulationMeta {
                name: name.to_string(),
                created_at: SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                last_tick: 0,
                total_births: 0,
                total_deaths: 0,
                peak_population: 0,
                description: format!("Simulation '{}' managed by Rust Engine", name),
            };
            self.save_meta(name, &meta);
        }
    }

    pub fn list_simulations(&self) -> Vec<SimulationMeta> {
        let mut list = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.base_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if let Some(meta) = self.load_meta(&name) {
                        list.push(meta);
                    } else {
                        list.push(SimulationMeta {
                            name: name.clone(),
                            created_at: 0,
                            last_tick: 0,
                            total_births: 0,
                            total_deaths: 0,
                            peak_population: 0,
                            description: format!("Simulation '{}'", name),
                        });
                    }
                }
            }
        }
        list
    }

    pub fn save_config(&self, name: &str, config: &SimConfig) {
        let path = self.sim_path(name).join("config.json");
        if let Ok(json) = serde_json::to_string_pretty(config) {
            let _ = fs::write(path, json);
        }
    }

    pub fn load_config(&self, name: &str) -> Option<SimConfig> {
        let path = self.sim_path(name).join("config.json");
        if let Ok(content) = fs::read_to_string(path) {
            serde_json::from_str(&content).ok()
        } else {
            None
        }
    }

    pub fn save_meta(&self, name: &str, meta: &SimulationMeta) {
        let path = self.sim_path(name).join("meta.json");
        if let Ok(json) = serde_json::to_string_pretty(meta) {
            let _ = fs::write(path, json);
        }
    }

    pub fn load_meta(&self, name: &str) -> Option<SimulationMeta> {
        let path = self.sim_path(name).join("meta.json");
        if let Ok(content) = fs::read_to_string(path) {
            serde_json::from_str(&content).ok()
        } else {
            None
        }
    }

    // Append periodic history snapshot (called every 100-200 ticks)
    pub fn append_history(
        &self,
        name: &str,
        point: &HistoryPoint,
        species: &[SpeciesInfo],
    ) {
        let path = self.sim_path(name).join("history.jsonl");
        let entry = DetailedHistoryEntry {
            tick: point.tick,
            population: point.population,
            food_count: point.food_count,
            meat_count: point.meat_count,
            avg_eyes: point.avg_eyes,
            avg_jaw: point.avg_jaw,
            avg_radius: point.avg_radius,
            herbivores: point.herbivores,
            carnivores: point.carnivores,
            max_generation: point.max_generation,
            species: species.to_vec(),
        };

        if let Ok(json_line) = serde_json::to_string(&entry) {
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
                let _ = writeln!(file, "{}", json_line);
            }
        }
    }

    // Append human-readable event log
    pub fn append_event(&self, name: &str, event: &SimEvent) {
        let path = self.sim_path(name).join("events.log");
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(file, "[Tick {:>7}] [{:<8}] {}", event.tick, event.level, event.message);
        }
    }

    pub fn read_recent_history(&self, name: &str, limit: usize) -> Vec<DetailedHistoryEntry> {
        let path = self.sim_path(name).join("history.jsonl");
        let mut entries = Vec::new();
        if let Ok(content) = fs::read_to_string(path) {
            for line in content.lines() {
                if let Ok(entry) = serde_json::from_str::<DetailedHistoryEntry>(line) {
                    entries.push(entry);
                }
            }
        }
        if entries.len() > limit {
            entries.split_off(entries.len() - limit)
        } else {
            entries
        }
    }
}
