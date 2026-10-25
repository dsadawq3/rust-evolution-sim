#![allow(dead_code)]
use crate::config::SimConfig;
use crate::species::SpeciesInfo;
use crate::telemetry::SimEvent;
use crate::world::{HistoryPoint, WorldStats};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
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
pub struct SimulationSummary {
    pub name: String,
    pub archived_at: u64,
    pub total_ticks_lived: u64,
    pub peak_population: usize,
    pub final_population: usize,
    pub total_births: u64,
    pub total_deaths: u64,
    pub max_generation: usize,
    pub dominant_species: String,
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChartTimelineData {
    pub world: String,
    pub total_points: usize,
    pub last_updated_tick: u64,
    pub ticks: Vec<u64>,
    pub population: Vec<usize>,
    pub food: Vec<usize>,
    pub meat: Vec<usize>,
    pub avg_eyes: Vec<f32>,
    pub avg_jaw: Vec<f32>,
    pub avg_radius: Vec<f32>,
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
    history_cache: Arc<RwLock<VecDeque<DetailedHistoryEntry>>>,
}

impl StorageManager {
    pub fn new(active_sim_name: &str) -> Self {
        let base_dir = PathBuf::from("simulations");
        let cache = Arc::new(RwLock::new(VecDeque::with_capacity(600)));
        let mgr = StorageManager {
            base_dir,
            active_sim: active_sim_name.to_string(),
            history_cache: cache,
        };
        mgr.ensure_sim_dir(active_sim_name);
        mgr.preload_cache(active_sim_name);
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
        self.preload_cache(name);
    }

    fn preload_cache(&self, name: &str) {
        let path = self.sim_path(name).join("history.jsonl");
        let mut entries = Vec::new();
        if let Ok(content) = fs::read_to_string(&path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if let Ok(entry) = serde_json::from_str::<DetailedHistoryEntry>(trimmed) {
                    entries.push(entry);
                }
            }
        }
        let mut cache = self.history_cache.write();
        cache.clear();
        let start = entries.len().saturating_sub(400);
        for entry in entries.into_iter().skip(start) {
            cache.push_back(entry);
        }
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

    pub fn get_unique_sim_name(&self, base_name: &str) -> String {
        let clean = base_name
            .trim()
            .replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '-', "_");
        let clean = if clean.is_empty() {
            "world".to_string()
        } else {
            clean
        };

        let path = self.sim_path(&clean);
        if !path.exists() {
            return clean;
        }

        let mut idx = 2;
        loop {
            let candidate = format!("{}_{}", clean, idx);
            if !self.sim_path(&candidate).exists() {
                return candidate;
            }
            idx += 1;
        }
    }

    pub fn reset_sim_files(&self, name: &str) {
        let path = self.sim_path(name);
        let _ = fs::create_dir_all(&path);
        let _ = fs::remove_file(path.join("history.jsonl"));
        let _ = fs::remove_file(path.join("timeline.json"));
        let _ = fs::remove_file(path.join("summary.json"));
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
            description: format!("Fresh simulation '{}'", name),
        };
        self.save_meta(name, &meta);
        let mut cache = self.history_cache.write();
        cache.clear();
    }

    pub fn delete_simulation(&self, name: &str) -> Result<(), String> {
        if name == self.active_sim {
            return Err("Нельзя удалить активную запущенную симуляцию! Сначала переключитесь на другую.".to_string());
        }
        let path = self.sim_path(name);
        if path.exists() {
            fs::remove_dir_all(&path).map_err(|e| e.to_string())?;
            Ok(())
        } else {
            Err("Симуляция не найдена на диске".to_string())
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

    // Append periodic history snapshot and maintain timeline.json on disk
    pub fn append_history(
        &self,
        name: &str,
        point: &HistoryPoint,
        species: &[SpeciesInfo],
    ) {
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

        if name == self.active_sim {
            let mut cache = self.history_cache.write();
            if cache.len() >= 500 {
                cache.pop_front();
            }
            cache.push_back(entry.clone());

            // Save clean timeline.json on disk for easy human & script reading
            let tl = ChartTimelineData {
                world: name.to_string(),
                total_points: cache.len(),
                last_updated_tick: point.tick,
                ticks: cache.iter().map(|e| e.tick).collect(),
                population: cache.iter().map(|e| e.population).collect(),
                food: cache.iter().map(|e| e.food_count).collect(),
                meat: cache.iter().map(|e| e.meat_count).collect(),
                avg_eyes: cache.iter().map(|e| e.avg_eyes).collect(),
                avg_jaw: cache.iter().map(|e| e.avg_jaw).collect(),
                avg_radius: cache.iter().map(|e| e.avg_radius).collect(),
            };
            let timeline_path = self.sim_path(name).join("timeline.json");
            if let Ok(json) = serde_json::to_string_pretty(&tl) {
                let _ = fs::write(timeline_path, json);
            }
        }

        let path = self.sim_path(name).join("history.jsonl");
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
        if name == self.active_sim {
            let cache = self.history_cache.read();
            let skip = cache.len().saturating_sub(limit);
            return cache.iter().skip(skip).cloned().collect();
        }

        let path = self.sim_path(name).join("history.jsonl");
        let mut entries = Vec::new();
        if let Ok(content) = fs::read_to_string(path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if let Ok(entry) = serde_json::from_str::<DetailedHistoryEntry>(trimmed) {
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

    pub fn get_timeline(&self, name: &str) -> Option<ChartTimelineData> {
        let timeline_path = self.sim_path(name).join("timeline.json");
        if let Ok(content) = fs::read_to_string(timeline_path) {
            serde_json::from_str(&content).ok()
        } else {
            None
        }
    }

    // Safe Archive & Reset: creates concise summary.json & runs_archive.jsonl, wipes tick history
    pub fn archive_and_reset(&self, name: &str, stats: &WorldStats, max_gen: usize, dominant_spec: &str) {
        let summary = SimulationSummary {
            name: name.to_string(),
            archived_at: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            total_ticks_lived: stats.tick,
            peak_population: stats.population,
            final_population: stats.population,
            total_births: stats.total_births,
            total_deaths: stats.total_deaths,
            max_generation: max_gen,
            dominant_species: dominant_spec.to_string(),
            description: format!(
                "Сводка забега: мир '{}' прожил {} тиков. Достигнуто поколение {}. Рождений: {}, Смертей: {}.",
                name, stats.tick, max_gen, stats.total_births, stats.total_deaths
            ),
        };

        // 1. Write summary.json
        let summary_path = self.sim_path(name).join("summary.json");
        if let Ok(json) = serde_json::to_string_pretty(&summary) {
            let _ = fs::write(summary_path, json);
        }

        // 2. Append to runs_archive.jsonl
        let archive_path = self.sim_path(name).join("runs_archive.jsonl");
        if let Ok(line) = serde_json::to_string(&summary) {
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(archive_path) {
                let _ = writeln!(file, "{}", line);
            }
        }

        // 3. Clear detailed history.jsonl and timeline.json for new run
        let hist_path = self.sim_path(name).join("history.jsonl");
        let _ = fs::write(hist_path, "");

        let timeline_path = self.sim_path(name).join("timeline.json");
        let empty_tl = ChartTimelineData {
            world: name.to_string(),
            total_points: 0,
            last_updated_tick: 0,
            ticks: Vec::new(),
            population: Vec::new(),
            food: Vec::new(),
            meat: Vec::new(),
            avg_eyes: Vec::new(),
            avg_jaw: Vec::new(),
            avg_radius: Vec::new(),
        };
        if let Ok(json) = serde_json::to_string_pretty(&empty_tl) {
            let _ = fs::write(timeline_path, json);
        }

        // 4. Reset events.log with header
        let events_path = self.sim_path(name).join("events.log");
        let _ = fs::write(events_path, format!("[Tick 0] [RESET] Симуляция '{}' сброшена. Краткий архив сохранен в summary.json.\n", name));

        // 5. Reset in-memory history cache
        if name == self.active_sim {
            let mut cache = self.history_cache.write();
            cache.clear();
        }
    }
}
