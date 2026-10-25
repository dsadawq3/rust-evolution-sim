use crate::snapshot::{SnapshotBuilder, WorldSnapshot};
use crate::species::SpeciesAnalyzer;
use crate::storage::{SimulationMeta, StorageManager};
use crate::telemetry::Telemetry;
use crate::world::World;
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

pub struct SimEngine {
    pub world: Arc<RwLock<World>>,
    pub telemetry: Telemetry,
    pub storage: Arc<RwLock<StorageManager>>,
    pub snapshot_tx: broadcast::Sender<WorldSnapshot>,
}

impl SimEngine {
    pub fn new(
        world: Arc<RwLock<World>>,
        telemetry: Telemetry,
        storage: Arc<RwLock<StorageManager>>,
        snapshot_tx: broadcast::Sender<WorldSnapshot>,
    ) -> Self {
        SimEngine {
            world,
            telemetry,
            storage,
            snapshot_tx,
        }
    }

    pub async fn run_loop(self) {
        let mut last_snapshot_time = Instant::now();
        let mut last_disk_flush_tick = 0u64;
        let mut accumulator = 0.0f32;
        let mut last_instant = Instant::now();

        loop {
            let now = Instant::now();
            let dt = (now - last_instant).as_secs_f32().min(0.2);
            last_instant = now;

            let (time_scale, is_paused) = {
                let w = self.world.read();
                (w.config.time_scale, w.config.is_paused)
            };

            if !is_paused && time_scale > 0.0 {
                accumulator += dt * time_scale;
                let tick_step = 1.0 / 40.0f32;

                let max_steps = if time_scale >= 50.0 { 150 } else { 25 };
                let mut steps = 0;

                while accumulator >= tick_step && steps < max_steps {
                    {
                        let mut w = self.world.write();
                        w.step(Some(&self.telemetry));
                    }
                    accumulator -= tick_step;
                    steps += 1;
                }

                if accumulator > tick_step * 3.0 {
                    accumulator = 0.0;
                }
            }

            // Periodic history logging to disk (every 120 ticks, guaranteed no disk thrashing)
            let current_tick = {
                let w = self.world.read();
                w.tick
            };

            if current_tick >= last_disk_flush_tick + 120 {
                last_disk_flush_tick = current_tick;
                let (stats, history_point, species) = {
                    let w = self.world.read();
                    let stats = w.get_stats();
                    let pt = w.history.last().cloned().unwrap_or(crate::world::HistoryPoint {
                        tick: stats.tick,
                        population: stats.population,
                        food_count: stats.food_count,
                        meat_count: stats.meat_count,
                        avg_eyes: stats.avg_eyes,
                        avg_jaw: stats.avg_jaw,
                        avg_radius: stats.avg_radius,
                        herbivores: stats.herbivores,
                        carnivores: stats.carnivores,
                        max_generation: stats.max_generation,
                    });
                    let spec = SpeciesAnalyzer::classify_population(&w.creatures);
                    (stats, pt, spec)
                };

                let storage = self.storage.read();
                let sim_name = storage.get_active_name();
                storage.append_history(&sim_name, &history_point, &species);

                let meta = SimulationMeta {
                    name: sim_name.clone(),
                    created_at: 0,
                    last_tick: stats.tick,
                    total_births: stats.total_births,
                    total_deaths: stats.total_deaths,
                    peak_population: stats.population,
                    description: format!("Active: {} creatures, {} species", stats.population, species.len()),
                };
                storage.save_meta(&sim_name, &meta);
            }

            // Periodic snapshot broadcast for Web UI (at ~30 FPS)
            if last_snapshot_time.elapsed() >= Duration::from_millis(33) {
                last_snapshot_time = Instant::now();
                let tps = self.telemetry.update_tps(current_tick);
                let (active_name, available_sims) = {
                    let s = self.storage.read();
                    let active = s.get_active_name();
                    let list = s.list_simulations().into_iter().map(|m| m.name).collect();
                    (active, list)
                };
                let recent_events = self.telemetry.get_recent_events();

                let snapshot = {
                    let w = self.world.read();
                    SnapshotBuilder::build(&w, tps, &active_name, available_sims, recent_events)
                };
                let _ = self.snapshot_tx.send(snapshot);
            }

            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }

    pub fn run_benchmark(&self, target_ticks: u64) {
        println!(">>> Starting headless benchmark for {} ticks...", target_ticks);
        let start = Instant::now();

        let mut w = self.world.write();
        for i in 1..=target_ticks {
            w.step(Some(&self.telemetry));
            if i % 1000 == 0 {
                let stats = w.get_stats();
                let elapsed = start.elapsed().as_secs_f32();
                let tps = i as f32 / elapsed.max(0.001);
                println!(
                    "  [Tick {:>6}] Pop: {:>4} | Food: {:>4} | Meat: {:>3} | Gen: {:>2} | Avg Eyes: {:.1} | Jaw: {:.2} | Speed: {:.0} TPS",
                    i, stats.population, stats.food_count, stats.meat_count, stats.max_generation, stats.avg_eyes, stats.avg_jaw, tps
                );
            }
        }

        let total_time = start.elapsed().as_secs_f32();
        println!(
            ">>> Benchmark finished! Total: {:.2}s, Average: {:.0} TPS",
            total_time,
            target_ticks as f32 / total_time
        );
    }
}
