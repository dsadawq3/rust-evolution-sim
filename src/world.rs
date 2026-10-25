use crate::chemical::{ChemicalGrid, SCENT_DANGER, SCENT_PLANT};
use crate::config::SimConfig;
use crate::creature::{Creature, PerceptInfo};
use crate::math::Vec2;
use crate::spatial::{EntityKind, SpatialEntry, SpatialGrid};
use crate::telemetry::Telemetry;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct FoodPellet {
    pub id: usize,
    pub pos: Vec2,
    pub energy: f32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct MeatChunk {
    pub id: usize,
    pub pos: Vec2,
    pub radius: f32,
    pub remaining_energy: f32,
    pub max_energy: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryPoint {
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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldStats {
    pub tick: u64,
    pub population: usize,
    pub food_count: usize,
    pub meat_count: usize,
    pub total_births: u64,
    pub total_deaths: u64,
    pub starvation_deaths: u64,
    pub age_deaths: u64,
    pub predation_deaths: u64,
    pub max_generation: usize,
    pub avg_eyes: f32,
    pub avg_jaw: f32,
    pub avg_radius: f32,
    pub avg_speed: f32,
    pub herbivores: usize,
    pub carnivores: usize,
    pub omnivores: usize,
    pub total_energy_pool: f32,
}

pub struct World {
    pub config: SimConfig,
    pub creatures: Vec<Creature>,
    pub food: Vec<FoodPellet>,
    pub meat: Vec<MeatChunk>,
    pub spatial_grid: SpatialGrid,
    pub chemical_grid: ChemicalGrid,

    pub tick: u64,
    pub next_id: usize,
    pub next_food_id: usize,
    pub next_meat_id: usize,

    pub total_births: u64,
    pub total_deaths: u64,
    pub starvation_deaths: u64,
    pub age_deaths: u64,
    pub predation_deaths: u64,

    pub history: Vec<HistoryPoint>,
    pub rng: SmallRng,
}

impl World {
    pub fn new(config: SimConfig) -> Self {
        let mut rng = SmallRng::from_entropy();
        let spatial_grid = SpatialGrid::new(config.world_width, config.world_height, config.grid_cell_size);
        let chemical_grid = ChemicalGrid::new(
            config.chemical_grid_width,
            config.chemical_grid_height,
            config.world_width,
            config.world_height,
        );

        let mut creatures = Vec::with_capacity(config.max_creatures);
        let mut next_id = 1;
        for _ in 0..config.initial_creatures {
            let pos = Vec2::new(
                rng.gen_range(15.0..config.world_width - 15.0),
                rng.gen_range(15.0..config.world_height - 15.0),
            );
            creatures.push(Creature::new_random(next_id, pos, &config, &mut rng));
            next_id += 1;
        }

        let mut food = Vec::with_capacity(config.max_food);
        let mut next_food_id = 1;
        for _ in 0..config.initial_food {
            let pos = Vec2::new(
                rng.gen_range(8.0..config.world_width - 8.0),
                rng.gen_range(8.0..config.world_height - 8.0),
            );
            food.push(FoodPellet {
                id: next_food_id,
                pos,
                energy: config.food_energy,
            });
            next_food_id += 1;
        }

        World {
            config,
            creatures,
            food,
            meat: Vec::new(),
            spatial_grid,
            chemical_grid,
            tick: 0,
            next_id,
            next_food_id,
            next_meat_id: 1,
            total_births: 0,
            total_deaths: 0,
            starvation_deaths: 0,
            age_deaths: 0,
            predation_deaths: 0,
            history: Vec::with_capacity(600),
            rng,
        }
    }

    pub fn step(&mut self, telemetry: Option<&Telemetry>) {
        if self.config.is_paused {
            return;
        }

        self.tick += 1;

        // 1. Rebuild Spatial Grid
        self.spatial_grid.clear();
        for (idx, c) in self.creatures.iter().enumerate() {
            if c.is_alive {
                self.spatial_grid.insert(SpatialEntry {
                    kind: EntityKind::Creature,
                    id: idx,
                    pos: c.pos,
                    radius: c.phenotype.body_radius,
                });
            }
        }
        for (idx, f) in self.food.iter().enumerate() {
            self.spatial_grid.insert(SpatialEntry {
                kind: EntityKind::Food,
                id: idx,
                pos: f.pos,
                radius: 0.8,
            });
        }
        for (idx, m) in self.meat.iter().enumerate() {
            self.spatial_grid.insert(SpatialEntry {
                kind: EntityKind::Meat,
                id: idx,
                pos: m.pos,
                radius: m.radius,
            });
        }

        // 2. Diffuse & Decay Chemicals + Plant odor
        if self.tick % 2 == 0 {
            self.chemical_grid.update();
        }
        if self.tick % 4 == 0 {
            for f in &self.food {
                self.chemical_grid.deposit(f.pos, SCENT_PLANT, 0.16);
            }
        }

        // 3. Parallel Perception & CTRNN Forward Pass (Rayon)
        let percept_table: Vec<PerceptInfo> = self
            .creatures
            .iter()
            .map(|c| c.get_percept_info())
            .collect();

        let grid_ref = &self.spatial_grid;
        let chem_ref = &self.chemical_grid;
        let table_ref = &percept_table;

        self.creatures.par_iter_mut().for_each(|c| {
            if c.is_alive {
                c.perceive_and_think_parallel(grid_ref, chem_ref, table_ref);
            }
        });

        // 4. Creature Physics & Metabolism Update
        for c in &mut self.creatures {
            if c.is_alive {
                c.update_physics_and_metabolism(&self.config, &mut self.chemical_grid);
            }
        }

        // 5. Environmental Interactions (Eating, Combat, Mating)
        let mut food_to_remove = std::collections::HashSet::new();
        let mut children_to_spawn: Vec<Creature> = Vec::new();

        let num_creatures = self.creatures.len();
        for i in 0..num_creatures {
            if !self.creatures[i].is_alive {
                continue;
            }

            let c_pos = self.creatures[i].pos;
            let c_radius = self.creatures[i].phenotype.body_radius;
            let c_jaw = self.creatures[i].phenotype.jaw_strength;
            let c_action = self.creatures[i].last_action;
            let reach = c_radius + 1.25;

            let neighbors = self.spatial_grid.query_radius(c_pos, reach);

            for entry in neighbors {
                match entry.kind {
                    EntityKind::Food => {
                        if !food_to_remove.contains(&entry.id) {
                            let food_item = &self.food[entry.id];
                            if c_pos.distance_squared(food_item.pos) <= (reach + 0.8).powi(2) {
                                let herbivore_eff = (1.0 - c_jaw * 0.72).clamp(0.12, 1.0);
                                let gained = food_item.energy * herbivore_eff;
                                self.creatures[i].energy = (self.creatures[i].energy + gained).min(self.creatures[i].max_energy);
                                self.creatures[i].food_eaten += 1;
                                food_to_remove.insert(entry.id);
                            }
                        }
                    }
                    EntityKind::Meat => {
                        if entry.id < self.meat.len() {
                            let meat = &mut self.meat[entry.id];
                            if meat.remaining_energy > 0.0 && c_pos.distance_squared(meat.pos) <= (reach + meat.radius).powi(2) {
                                let bite_amount = 16.0f32.min(meat.remaining_energy);
                                let carnivore_eff = (0.35 + c_jaw * 0.65).clamp(0.2, 1.0);
                                let gained = bite_amount * carnivore_eff * self.config.meat_energy_ratio;
                                self.creatures[i].energy = (self.creatures[i].energy + gained).min(self.creatures[i].max_energy);
                                self.creatures[i].meat_eaten += 1;
                                meat.remaining_energy -= bite_amount;
                            }
                        }
                    }
                    EntityKind::Creature => {
                        let j = entry.id;
                        if i == j || !self.creatures[j].is_alive {
                            continue;
                        }

                        let dist_sq = c_pos.distance_squared(self.creatures[j].pos);
                        let contact_dist = c_radius + self.creatures[j].phenotype.body_radius;

                        if dist_sq <= contact_dist * contact_dist {
                            // Soft physical separation
                            let dist = dist_sq.sqrt().max(0.01);
                            let overlap = contact_dist - dist;
                            let normal = (c_pos - self.creatures[j].pos) / dist;
                            self.creatures[i].pos += normal * (overlap * 0.35);
                            self.creatures[i].contact_impulse = 0.6;

                            // Attack / Combat action with biological satiety and stamina cost
                            if c_action.bite && c_jaw > 0.15 {
                                let is_hungry = self.creatures[i].energy < self.creatures[i].max_energy * 0.85;
                                let bite_cost = self.config.bite_cost * (0.8 + c_jaw * 1.4);

                                if is_hungry && self.creatures[i].energy > bite_cost {
                                    self.creatures[i].energy -= bite_cost;

                                    let victim_armor = self.creatures[j].phenotype.armor_spikes;
                                    let damage = (c_jaw * 26.0 * (1.0 - victim_armor * 0.82)).max(0.5);
                                    self.creatures[j].health -= damage;
                                    self.creatures[j].last_damage_taken = damage;

                                    let blood_calories = damage * 0.40 * (0.35 + c_jaw * 0.65);
                                    self.creatures[i].energy = (self.creatures[i].energy + blood_calories).min(self.creatures[i].max_energy);
                                    self.chemical_grid.deposit(self.creatures[j].pos, SCENT_DANGER, 1.8);

                                    // Strong armor & spikes counter-defense (dissuades reckless predators)
                                    if victim_armor > 0.08 {
                                        let reflect = damage * (0.35 + victim_armor * 0.85);
                                        self.creatures[i].health -= reflect;
                                        self.creatures[i].energy = (self.creatures[i].energy - reflect * 0.5).max(0.0);
                                    }

                                    if self.creatures[j].health <= 0.0 {
                                        self.creatures[j].is_alive = false;
                                        self.creatures[i].kills += 1;
                                        self.predation_deaths += 1;
                                        if let Some(t) = telemetry {
                                            t.log(self.tick, "COMBAT", format!("Хищник #{} добыл жертву #{} (челюсть: {:.2})", self.creatures[i].id, self.creatures[j].id, c_jaw));
                                        }
                                    }
                                }
                            }

                            // Natural reproduction (courtship & mutual attraction)
                            if (c_action.mate || self.creatures[j].last_action.mate)
                                && self.creatures[i].is_fertile()
                                && self.creatures[j].is_fertile()
                                && self.creatures.len() + children_to_spawn.len() < self.config.max_creatures
                            {
                                let attr_ij = self.creatures[i].calculate_attraction(&self.creatures[j]);
                                let attr_ji = self.creatures[j].calculate_attraction(&self.creatures[i]);

                                if attr_ij > 0.03 && attr_ji > 0.03 {
                                    let clutch = self.creatures[i].phenotype.clutch_size.min(2);
                                    for _ in 0..clutch {
                                        let offset = Vec2::new(
                                            self.rng.gen_range(-2.0..2.0),
                                            self.rng.gen_range(-2.0..2.0),
                                        );
                                        let child_pos = c_pos + offset;
                                        let child = Creature::new_child(
                                            self.next_id,
                                            child_pos,
                                            &self.creatures[i],
                                            &self.creatures[j],
                                            &self.config,
                                            &mut self.rng,
                                        );
                                        self.next_id += 1;
                                        children_to_spawn.push(child);
                                        self.total_births += 1;
                                    }

                                    self.creatures[i].energy *= 0.65;
                                    self.creatures[j].energy *= 0.70;
                                    self.creatures[i].reproduction_cooldown = 110.0;
                                    self.creatures[j].reproduction_cooldown = 110.0;
                                    self.creatures[i].offspring_count += clutch as u32;
                                    self.creatures[j].offspring_count += clutch as u32;
                                }
                            }
                        }
                    }
                }
            }
        }

        // 6. Food removal
        if !food_to_remove.is_empty() {
            let mut indices: Vec<usize> = food_to_remove.into_iter().collect();
            indices.sort_unstable_by(|a, b| b.cmp(a));
            for idx in indices {
                if idx < self.food.len() {
                    self.food.swap_remove(idx);
                }
            }
        }

        // 7. Dead Creatures into Meat Carcasses (Conservation of Biomass)
        let mut new_meat = Vec::new();
        for c in &self.creatures {
            if !c.is_alive {
                self.total_deaths += 1;
                if c.energy <= 0.0 {
                    self.starvation_deaths += 1;
                } else if c.age >= c.phenotype.lifespan {
                    self.age_deaths += 1;
                }

                // 100% biomass conserved into meat chunks
                let carcass_energy = c.phenotype.mass * 28.0 + c.energy * 0.5;
                new_meat.push(MeatChunk {
                    id: self.next_meat_id,
                    pos: c.pos,
                    radius: c.phenotype.body_radius * 0.85,
                    remaining_energy: carcass_energy,
                    max_energy: carcass_energy,
                });
                self.next_meat_id += 1;
            }
        }
        self.meat.extend(new_meat);

        self.creatures.retain(|c| c.is_alive);
        self.creatures.extend(children_to_spawn);

        // 8. Meat Decay
        let decay = self.config.meat_decay_rate;
        for m in &mut self.meat {
            m.remaining_energy -= m.max_energy * decay;
        }
        self.meat.retain(|m| m.remaining_energy > 1.0);

        // 9. Plant Food Spawning
        let food_deficit = (self.config.max_food as isize - self.food.len() as isize).max(0) as usize;
        if food_deficit > 0 {
            let spawn_chance = (self.config.food_spawn_per_sec / self.config.ticks_per_sec as f32).min(10.0);
            let to_spawn = spawn_chance.floor() as usize + if self.rng.gen_bool(spawn_chance.fract() as f64) { 1 } else { 0 };

            for _ in 0..to_spawn.min(food_deficit) {
                let pos = if !self.food.is_empty() && self.rng.gen_bool(0.72) {
                    let parent_idx = self.rng.gen_range(0..self.food.len());
                    let parent_pos = self.food[parent_idx].pos;
                    let angle = self.rng.gen_range(0.0..std::f32::consts::TAU);
                    let dist = self.rng.gen_range(2.0..20.0);
                    Vec2::new(
                        (parent_pos.x + angle.cos() * dist).clamp(10.0, self.config.world_width - 10.0),
                        (parent_pos.y + angle.sin() * dist).clamp(10.0, self.config.world_height - 10.0),
                    )
                } else {
                    Vec2::new(
                        self.rng.gen_range(10.0..self.config.world_width - 10.0),
                        self.rng.gen_range(10.0..self.config.world_height - 10.0),
                    )
                };

                self.food.push(FoodPellet {
                    id: self.next_food_id,
                    pos,
                    energy: self.config.food_energy,
                });
                self.next_food_id += 1;
            }
        }

        // 10. Extinction & Natural Ecosystem State
        if self.creatures.is_empty() {
            if let Some(t) = telemetry {
                if self.tick % 240 == 0 {
                    t.log(self.tick, "FATAL", "Популяция полностью вымерла (естественный коллапс). Нажмите 'Сброс' в меню.".to_string());
                }
            }
        } else if self.config.enable_pioneer_reseeding && self.creatures.len() < self.config.min_creatures {
            if let Some(t) = telemetry {
                t.log(self.tick, "WARN", format!("Популяция упала до {}. Подсадка пионеров.", self.creatures.len()));
            }
            for _ in 0..(self.config.min_creatures - self.creatures.len()) {
                let pos = Vec2::new(
                    self.rng.gen_range(20.0..self.config.world_width - 20.0),
                    self.rng.gen_range(20.0..self.config.world_height - 20.0),
                );
                self.creatures.push(Creature::new_random(self.next_id, pos, &self.config, &mut self.rng));
                self.next_id += 1;
            }
        }

        // 11. History Sampling every 40 ticks (~1 second)
        if self.tick % 40 == 0 {
            self.record_history();
        }
    }

    fn record_history(&mut self) {
        let stats = self.get_stats();
        self.history.push(HistoryPoint {
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

        if self.history.len() > 600 {
            self.history.remove(0);
        }
    }

    pub fn get_stats(&self) -> WorldStats {
        let pop = self.creatures.len();
        let mut sum_eyes = 0.0;
        let mut sum_jaw = 0.0;
        let mut sum_radius = 0.0;
        let mut sum_speed = 0.0;
        let mut max_gen = 0;
        let mut herbs = 0;
        let mut carns = 0;
        let mut omnis = 0;
        let mut total_creature_energy = 0.0;

        for c in &self.creatures {
            sum_eyes += c.phenotype.eye_count as f32;
            sum_jaw += c.phenotype.jaw_strength;
            sum_radius += c.phenotype.body_radius;
            sum_speed += c.phenotype.base_speed;
            total_creature_energy += c.energy;
            if c.generation > max_gen {
                max_gen = c.generation;
            }
            if c.phenotype.jaw_strength < 0.28 {
                herbs += 1;
            } else if c.phenotype.jaw_strength > 0.55 {
                carns += 1;
            } else {
                omnis += 1;
            }
        }

        let total_food_energy: f32 = self.food.iter().map(|f| f.energy).sum();
        let total_meat_energy: f32 = self.meat.iter().map(|m| m.remaining_energy).sum();
        let total_energy_pool = total_creature_energy + total_food_energy + total_meat_energy;

        let p_f = pop.max(1) as f32;
        WorldStats {
            tick: self.tick,
            population: pop,
            food_count: self.food.len(),
            meat_count: self.meat.len(),
            total_births: self.total_births,
            total_deaths: self.total_deaths,
            starvation_deaths: self.starvation_deaths,
            age_deaths: self.age_deaths,
            predation_deaths: self.predation_deaths,
            max_generation: max_gen,
            avg_eyes: sum_eyes / p_f,
            avg_jaw: sum_jaw / p_f,
            avg_radius: sum_radius / p_f,
            avg_speed: sum_speed / p_f,
            herbivores: herbs,
            carnivores: carns,
            omnivores: omnis,
            total_energy_pool,
        }
    }

    // God Controls
    pub fn disaster_meteor(&mut self) {
        let victims_count = (self.creatures.len() * 3) / 4;
        for _ in 0..victims_count {
            if self.creatures.is_empty() {
                break;
            }
            let idx = self.rng.gen_range(0..self.creatures.len());
            let mut c = self.creatures.swap_remove(idx);
            c.is_alive = false;
            let carcass_energy = c.phenotype.mass * 35.0;
            self.meat.push(MeatChunk {
                id: self.next_meat_id,
                pos: c.pos,
                radius: c.phenotype.body_radius * 0.9,
                remaining_energy: carcass_energy,
                max_energy: carcass_energy,
            });
            self.next_meat_id += 1;
            self.chemical_grid.deposit(c.pos, SCENT_DANGER, 3.5);
        }
    }

    pub fn radiation_burst(&mut self) {
        self.config.mutation_rate_multiplier = 8.0;
    }

    pub fn spawn_food_cluster(&mut self, count: usize) {
        let center = Vec2::new(
            self.rng.gen_range(30.0..self.config.world_width - 30.0),
            self.rng.gen_range(30.0..self.config.world_height - 30.0),
        );
        for _ in 0..count {
            let offset = Vec2::new(
                self.rng.gen_range(-25.0..25.0),
                self.rng.gen_range(-25.0..25.0),
            );
            self.food.push(FoodPellet {
                id: self.next_food_id,
                pos: center + offset,
                energy: self.config.food_energy,
            });
            self.next_food_id += 1;
        }
    }

    pub fn spawn_meat_bomb(&mut self, count: usize) {
        let center = Vec2::new(
            self.rng.gen_range(30.0..self.config.world_width - 30.0),
            self.rng.gen_range(30.0..self.config.world_height - 30.0),
        );
        for _ in 0..count {
            let offset = Vec2::new(
                self.rng.gen_range(-20.0..20.0),
                self.rng.gen_range(-20.0..20.0),
            );
            self.meat.push(MeatChunk {
                id: self.next_meat_id,
                pos: center + offset,
                radius: self.rng.gen_range(1.0..2.2),
                remaining_energy: 120.0,
                max_energy: 120.0,
            });
            self.next_meat_id += 1;
        }
    }
}
