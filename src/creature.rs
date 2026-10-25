#![allow(dead_code)]
use crate::brain::{Brain, MotorOutputs};
use crate::chemical::{ChemicalGrid, SCENT_DANGER, SCENT_MATE, SCENT_PLANT};
use crate::config::SimConfig;
use crate::genetics::{DiploidGenome, Phenotype, BRAIN_INPUT_SIZE};
use crate::math::{normalize_angle, Vec2};
use crate::spatial::{EntityKind, SpatialGrid};
use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug)]
pub struct PerceptInfo {
    pub id: usize,
    pub pos: Vec2,
    pub radius: f32,
    pub jaw_strength: f32,
    pub is_fertile: bool,
    pub color_hue: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Creature {
    pub id: usize,
    pub generation: usize,
    pub lineage_id: usize,
    pub genome: DiploidGenome,
    pub phenotype: Phenotype,

    pub pos: Vec2,
    pub vel: Vec2,
    pub rotation: f32, // Heading in radians

    pub energy: f32,
    pub max_energy: f32,
    pub health: f32,
    pub age: f32,
    pub is_alive: bool,

    pub brain: Brain,
    pub last_action: MotorOutputs,
    pub reproduction_cooldown: f32,

    pub kills: u32,
    pub offspring_count: u32,
    pub food_eaten: u32,
    pub meat_eaten: u32,
    pub last_damage_taken: f32,
    pub contact_impulse: f32,
}

impl Creature {
    pub fn new_random<R: Rng>(id: usize, pos: Vec2, _config: &SimConfig, rng: &mut R) -> Self {
        let genome = DiploidGenome::random_new(rng);
        let phenotype = genome.express_phenotype();
        let max_energy = phenotype.body_radius * 130.0;
        let health = phenotype.max_health;
        let rotation = rng.gen_range(-std::f32::consts::PI..std::f32::consts::PI);

        Creature {
            id,
            generation: 1,
            lineage_id: id,
            genome,
            phenotype,
            pos,
            vel: Vec2::ZERO,
            rotation,
            energy: max_energy * 0.75,
            max_energy,
            health,
            age: 0.0,
            is_alive: true,
            brain: Brain::new(),
            last_action: MotorOutputs::default(),
            reproduction_cooldown: 50.0,
            kills: 0,
            offspring_count: 0,
            food_eaten: 0,
            meat_eaten: 0,
            last_damage_taken: 0.0,
            contact_impulse: 0.0,
        }
    }

    pub fn new_child<R: Rng>(
        id: usize,
        pos: Vec2,
        mother: &Creature,
        father: &Creature,
        config: &SimConfig,
        rng: &mut R,
    ) -> Self {
        let child_genome = DiploidGenome::crossover_and_mutate(
            &mother.genome,
            &father.genome,
            config.mutation_rate_multiplier,
            rng,
        );
        let phenotype = child_genome.express_phenotype();
        let max_energy = phenotype.body_radius * 130.0;
        let starting_energy = (max_energy * 0.5).min(mother.energy * mother.phenotype.energy_investment);
        let rotation = rng.gen_range(-std::f32::consts::PI..std::f32::consts::PI);

        Creature {
            id,
            generation: mother.generation.max(father.generation) + 1,
            lineage_id: mother.lineage_id,
            genome: child_genome,
            phenotype,
            pos,
            vel: Vec2::ZERO,
            rotation,
            energy: starting_energy.max(25.0),
            max_energy,
            health: 40.0,
            age: 0.0,
            is_alive: true,
            brain: Brain::new(),
            last_action: MotorOutputs::default(),
            reproduction_cooldown: 80.0,
            kills: 0,
            offspring_count: 0,
            food_eaten: 0,
            meat_eaten: 0,
            last_damage_taken: 0.0,
            contact_impulse: 0.0,
        }
    }

    pub fn get_percept_info(&self) -> PerceptInfo {
        PerceptInfo {
            id: self.id,
            pos: self.pos,
            radius: self.phenotype.body_radius,
            jaw_strength: self.phenotype.jaw_strength,
            is_fertile: self.is_fertile(),
            color_hue: self.phenotype.color_hue,
        }
    }

    pub fn is_fertile(&self) -> bool {
        let maturity_age = self.phenotype.lifespan * self.phenotype.maturity_age_ratio;
        self.age >= maturity_age && self.energy >= self.max_energy * 0.55 && self.reproduction_cooldown <= 0.0
    }

    // "Хорни-старушки": choosiness drops as creature nears the end of its life
    pub fn current_choosiness(&self) -> f32 {
        let age_ratio = (self.age / self.phenotype.lifespan).clamp(0.0, 1.0);
        let decay = (1.0 - age_ratio * age_ratio).max(0.04);
        self.phenotype.base_choosiness * decay
    }

    pub fn calculate_attraction(&self, other: &Creature) -> f32 {
        if !other.is_alive {
            return 0.0;
        }

        let hue_diff = (self.genome.parent_imprint_hue - other.phenotype.color_hue).abs();
        let hue_dist = hue_diff.min(1.0 - hue_diff);
        let radius_dist = (self.genome.parent_imprint_radius - other.phenotype.body_radius).abs() / 2.0;

        let total_pheno_dist = hue_dist + radius_dist;

        let choosiness = self.current_choosiness();
        let tolerance = (1.0 - choosiness) * 0.75 + 0.25;

        if total_pheno_dist <= tolerance {
            1.0 - (total_pheno_dist / tolerance)
        } else {
            0.0
        }
    }

    pub fn calculate_attraction_percept(&self, other: &PerceptInfo) -> f32 {
        let hue_diff = (self.genome.parent_imprint_hue - other.color_hue).abs();
        let hue_dist = hue_diff.min(1.0 - hue_diff);
        let radius_dist = (self.genome.parent_imprint_radius - other.radius).abs() / 2.0;

        let total_pheno_dist = hue_dist + radius_dist;

        let choosiness = self.current_choosiness();
        let tolerance = (1.0 - choosiness) * 0.75 + 0.25;

        if total_pheno_dist <= tolerance {
            1.0 - (total_pheno_dist / tolerance)
        } else {
            0.0
        }
    }

    pub fn perceive_and_think_parallel(
        &mut self,
        grid: &SpatialGrid,
        chemicals: &ChemicalGrid,
        all_creatures: &[PerceptInfo],
    ) {
        let mut inputs = [0.0f32; BRAIN_INPUT_SIZE];

        for (i, eye) in self.phenotype.eyes.iter().enumerate().take(6) {
            let world_eye_angle = normalize_angle(self.rotation + eye.azimuth);
            let in_idx = i * 2;

            if let Some((entry, dist)) = grid.query_cone(self.pos, world_eye_angle, eye.fov, eye.range, Some(self.id)) {
                let norm_dist = (1.0 - (dist / eye.range)).clamp(0.0, 1.0);
                inputs[in_idx] = norm_dist;

                let type_signal = match entry.kind {
                    EntityKind::Food => 0.5,
                    EntityKind::Meat => 0.85,
                    EntityKind::Creature => {
                        if let Some(target) = all_creatures.get(entry.id) {
                            if self.phenotype.jaw_strength > 0.4 && target.radius < self.phenotype.body_radius * 1.15 {
                                1.0 // Prey
                            } else if target.jaw_strength > 0.4 && target.radius > self.phenotype.body_radius * 0.95 {
                                -1.0 // Danger
                            } else if self.is_fertile() && target.is_fertile {
                                self.calculate_attraction_percept(target) * 0.6 + 0.2
                            } else {
                                0.1
                            }
                        } else {
                            0.0
                        }
                    }
                };
                inputs[in_idx + 1] = type_signal;
            } else {
                inputs[in_idx] = 0.0;
                inputs[in_idx + 1] = 0.0;
            }
        }

        inputs[12] = (self.energy / self.max_energy).clamp(0.0, 1.0);
        inputs[13] = (self.health / self.phenotype.max_health).clamp(0.0, 1.0);
        inputs[14] = (self.age / self.phenotype.lifespan).clamp(0.0, 1.0);
        inputs[15] = chemicals.sample(self.pos, SCENT_PLANT).clamp(0.0, 1.0);
        inputs[16] = chemicals.sample(self.pos, SCENT_DANGER).clamp(0.0, 1.0);
        inputs[17] = chemicals.sample(self.pos, SCENT_MATE).clamp(0.0, 1.0);
        inputs[18] = self.contact_impulse.clamp(-1.0, 1.0);
        inputs[19] = (self.vel.length() / self.phenotype.base_speed).clamp(0.0, 1.0);

        self.contact_impulse = 0.0;
        self.last_action = self.brain.forward(&inputs, &self.phenotype.brain_weights);
    }

    pub fn update_physics_and_metabolism(&mut self, config: &SimConfig, chemicals: &mut ChemicalGrid) {
        if !self.is_alive {
            return;
        }

        self.age += 1.0;
        if self.reproduction_cooldown > 0.0 {
            self.reproduction_cooldown -= 1.0;
        }

        let turn_rate = self.last_action.steer * 0.11;
        self.rotation = normalize_angle(self.rotation + turn_rate);

        let forward_dir = Vec2::from_angle(self.rotation);
        let thrust = self.last_action.throttle * self.phenotype.base_speed * 0.22;
        let accel = forward_dir * (thrust / self.phenotype.mass);
        self.vel += accel;

        self.vel = self.vel * 0.88;
        let max_speed = self.phenotype.base_speed * 0.65;
        let cur_spd_sq = self.vel.length_squared();
        if cur_spd_sq > max_speed * max_speed && cur_spd_sq > 0.0001 {
            self.vel = self.vel * (max_speed / cur_spd_sq.sqrt());
        }
        self.pos += self.vel;

        let margin = self.phenotype.body_radius;
        if self.pos.x < margin {
            self.pos.x = margin;
            self.vel.x = self.vel.x.abs() * 0.5;
            self.contact_impulse = 0.5;
        } else if self.pos.x > config.world_width - margin {
            self.pos.x = config.world_width - margin;
            self.vel.x = -self.vel.x.abs() * 0.5;
            self.contact_impulse = 0.5;
        }

        if self.pos.y < margin {
            self.pos.y = margin;
            self.vel.y = self.vel.y.abs() * 0.5;
            self.contact_impulse = 0.5;
        } else if self.pos.y > config.world_height - margin {
            self.pos.y = config.world_height - margin;
            self.vel.y = -self.vel.y.abs() * 0.5;
            self.contact_impulse = 0.5;
        }

        let base_burn = config.base_metabolism * self.phenotype.body_radius * (1.0 / self.phenotype.metabolic_efficiency);
        let move_speed = self.vel.length();
        let move_burn = move_speed * move_speed * config.movement_energy_cost * self.phenotype.mass;
        let eye_burn = self.phenotype.eye_count as f32 * config.eye_maintenance_cost;
        let armor_burn = self.phenotype.armor_spikes * config.armor_maintenance_cost;

        let total_burn = base_burn + move_burn + eye_burn + armor_burn;
        self.energy -= total_burn;

        if self.energy > self.max_energy * 0.70 && self.health < self.phenotype.max_health {
            let heal_rate = 0.08 * self.phenotype.metabolic_efficiency;
            self.health = (self.health + heal_rate).min(self.phenotype.max_health);
            self.energy -= heal_rate * 0.5;
        }

        if self.last_action.emit_pheromone > 0.3 {
            let channel = if self.is_fertile() {
                SCENT_MATE
            } else {
                SCENT_PLANT
            };
            chemicals.deposit(self.pos, channel, self.last_action.emit_pheromone * 0.4);
        }

        if self.energy <= 0.0 || self.health <= 0.0 || self.age >= self.phenotype.lifespan {
            self.is_alive = false;
            chemicals.deposit(self.pos, SCENT_DANGER, 1.8);
        }
    }
}
