use crate::snapshot::{SnapshotBuilder, WorldSnapshot};
use crate::telemetry::Telemetry;
use crate::world::World;
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

pub struct SimEngine {
    pub world: Arc<RwLock<World>>,
    pub telemetry: Telemetry,
    pub snapshot_tx: broadcast::Sender<WorldSnapshot>,
}

impl SimEngine {
    pub fn new(world: Arc<RwLock<World>>, telemetry: Telemetry, snapshot_tx: broadcast::Sender<WorldSnapshot>) -> Self {
        SimEngine {
            world,
            telemetry,
            snapshot_tx,
        }
    }

    pub async fn run_loop(self) {
        let mut last_snapshot_time = Instant::now();
        let mut accumulator = 0.0f32;
        let mut last_instant = Instant::now();

        loop {
            let now = Instant::now();
            let dt = (now - last_instant).as_secs_f32().min(0.2); // cap frame spikes
            last_instant = now;

            let (time_scale, is_paused) = {
                let w = self.world.read();
                (w.config.time_scale, w.config.is_paused)
            };

            if !is_paused && time_scale > 0.0 {
                accumulator += dt * time_scale;
                let tick_step = 1.0 / 40.0f32;

                // Max steps per frame to ensure fair execution without stalling
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

            // Periodic snapshot broadcast for Web UI
            if last_snapshot_time.elapsed() >= Duration::from_millis(33) {
                last_snapshot_time = Instant::now();
                let current_tick = {
                    let w = self.world.read();
                    w.tick
                };
                let tps = self.telemetry.update_tps(current_tick);
                let snapshot = {
                    let w = self.world.read();
                    SnapshotBuilder::build(&w, tps)
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
