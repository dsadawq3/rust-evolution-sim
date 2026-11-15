use crate::creature::Creature;
use crate::species::{cluster_of, diet_of, SpeciesAnalyzer, SpeciesInfo};
use crate::telemetry::SimEvent;
use crate::world::{World, WorldStats};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct CreatureRenderData {
    pub id: usize,
    pub x: f32,
    pub y: f32,
    pub rot: f32,
    pub r: f32,
    pub hue: f32,
    pub sat: f32,
    pub nrg: f32,
    pub hp: f32,
    pub jaw: f32,
    pub spk: f32,
    /// [azimuth, elevation, range, fov]
    pub eyes: Vec<[f32; 4]>,
    pub gen: usize,
    pub line: usize,
    pub age: f32,
    pub diet: u8,
    pub cluster: u32,
    /// bit 1 bite, bit 2 mate, bit 4 fertile, bit 8 thrusting
    pub act: u8,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct WorldSnapshot {
    pub tick: u64,
    pub tps: f32,
    pub stats: WorldStats,
    pub creatures: Vec<CreatureRenderData>,
    pub food: Vec<[f32; 2]>,
    pub meat: Vec<[f32; 4]>, // [x, y, radius, energy_ratio]
    pub species: Vec<SpeciesInfo>,
    pub active_sim: String,
    pub available_sims: Vec<String>,
    pub recent_events: Vec<SimEvent>,
    pub world_w: f32,
    pub world_h: f32,
    pub paused: bool,
    pub speed: f32,
    pub mut_rate: f32,
    pub scent_w: usize,
    pub scent_h: usize,
    pub scent: Vec<u8>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CreatureDetail {
    pub id: usize,
    pub generation: usize,
    pub lineage_id: usize,
    pub age: f32,
    pub lifespan: f32,
    pub energy: f32,
    pub max_energy: f32,
    pub health: f32,
    pub max_health: f32,
    pub speed: f32,
    pub radius: f32,
    pub jaw_strength: f32,
    pub armor_spikes: f32,
    pub metabolic_eff: f32,
    pub kills: u32,
    pub offspring: u32,
    pub food_eaten: u32,
    pub meat_eaten: u32,
    pub choosiness: f32,
    pub eye_count: usize,
    pub brain_inputs: Vec<f32>,
    pub brain_hidden: Vec<f32>,
    pub brain_outputs: Vec<f32>,
}

pub struct SnapshotBuilder;

impl SnapshotBuilder {
    pub fn build(
        world: &World,
        tps: f32,
        active_sim: &str,
        available_sims: Vec<String>,
        recent_events: Vec<SimEvent>,
    ) -> WorldSnapshot {
        let creatures: Vec<CreatureRenderData> = world
            .creatures
            .iter()
            .filter(|c| c.is_alive)
            .map(|c| {
                let eyes = c
                    .phenotype
                    .eyes
                    .iter()
                    .map(|e| [e.azimuth, e.elevation, e.range, e.fov])
                    .collect();

                let mut act = 0u8;
                if c.last_action.bite {
                    act |= 1;
                }
                if c.last_action.mate {
                    act |= 2;
                }
                if c.is_fertile() {
                    act |= 4;
                }
                if c.last_action.throttle > 0.55 {
                    act |= 8;
                }

                CreatureRenderData {
                    id: c.id,
                    x: c.pos.x,
                    y: c.pos.y,
                    rot: c.rotation,
                    r: c.phenotype.body_radius,
                    hue: c.phenotype.color_hue,
                    sat: c.phenotype.color_saturation,
                    nrg: (c.energy / c.max_energy).clamp(0.0, 1.0),
                    hp: (c.health / c.phenotype.max_health).clamp(0.0, 1.0),
                    jaw: c.phenotype.jaw_strength,
                    spk: c.phenotype.armor_spikes,
                    eyes,
                    gen: c.generation,
                    line: c.lineage_id,
                    age: (c.age / c.phenotype.lifespan.max(1.0)).clamp(0.0, 1.0),
                    diet: diet_of(c.phenotype.jaw_strength),
                    cluster: cluster_of(c),
                    act,
                }
            })
            .collect();

        let food = world.food.iter().map(|f| [f.pos.x, f.pos.y]).collect();

        let meat = world
            .meat
            .iter()
            .map(|m| [m.pos.x, m.pos.y, m.radius, (m.remaining_energy / m.max_energy).clamp(0.0, 1.0)])
            .collect();

        let species = SpeciesAnalyzer::classify_population(&world.creatures);
        let (scent_w, scent_h, scent) = world.chemical_grid.preview(28);

        WorldSnapshot {
            tick: world.tick,
            tps,
            stats: world.get_stats(),
            creatures,
            food,
            meat,
            species,
            active_sim: active_sim.to_string(),
            available_sims,
            recent_events,
            world_w: world.config.world_width,
            world_h: world.config.world_height,
            paused: world.config.is_paused || world.config.time_scale <= 0.0,
            speed: world.config.time_scale,
            mut_rate: world.config.mutation_rate_multiplier,
            scent_w,
            scent_h,
            scent,
        }
    }

    pub fn build_creature_detail(c: &Creature) -> CreatureDetail {
        CreatureDetail {
            id: c.id,
            generation: c.generation,
            lineage_id: c.lineage_id,
            age: c.age,
            lifespan: c.phenotype.lifespan,
            energy: c.energy,
            max_energy: c.max_energy,
            health: c.health,
            max_health: c.phenotype.max_health,
            speed: c.phenotype.base_speed,
            radius: c.phenotype.body_radius,
            jaw_strength: c.phenotype.jaw_strength,
            armor_spikes: c.phenotype.armor_spikes,
            metabolic_eff: c.phenotype.metabolic_efficiency,
            kills: c.kills,
            offspring: c.offspring_count,
            food_eaten: c.food_eaten,
            meat_eaten: c.meat_eaten,
            choosiness: c.current_choosiness(),
            eye_count: c.phenotype.eye_count,
            brain_inputs: c.brain.last_inputs.to_vec(),
            brain_hidden: c.brain.hidden_state.to_vec(),
            brain_outputs: c.brain.last_outputs.to_vec(),
        }
    }
}
