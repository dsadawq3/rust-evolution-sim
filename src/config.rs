use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimConfig {
    pub world_width: f32,
    pub world_height: f32,
    pub grid_cell_size: f32,
    pub chemical_grid_width: usize,
    pub chemical_grid_height: usize,

    // Initial populations
    pub initial_creatures: usize,
    pub min_creatures: usize,
    pub max_creatures: usize,
    pub initial_food: usize,
    pub max_food: usize,
    pub food_spawn_per_sec: f32,

    // Energy economics
    pub food_energy: f32,
    pub meat_energy_ratio: f32,
    pub meat_decay_rate: f32,
    pub base_metabolism: f32,
    pub movement_energy_cost: f32,
    pub eye_maintenance_cost: f32,
    pub armor_maintenance_cost: f32,
    pub bite_cost: f32,
    pub min_reproduction_energy: f32,

    // Environmental / Genetics / Nature settings
    pub mutation_rate_multiplier: f32,
    pub enable_pioneer_reseeding: bool,
    pub time_scale: f32,
    pub is_paused: bool,
    pub ticks_per_sec: u32,
}

impl Default for SimConfig {
    fn default() -> Self {
        SimConfig {
            world_width: 280.0,
            world_height: 280.0,
            grid_cell_size: 14.0,
            chemical_grid_width: 70,
            chemical_grid_height: 70,

            initial_creatures: 140,
            min_creatures: 25,
            max_creatures: 1200,
            initial_food: 350,
            max_food: 900,
            food_spawn_per_sec: 25.0,

            food_energy: 40.0,
            meat_energy_ratio: 0.85,
            meat_decay_rate: 0.003,
            base_metabolism: 0.030,
            movement_energy_cost: 0.02,
            eye_maintenance_cost: 0.008,
            armor_maintenance_cost: 0.015,
            bite_cost: 1.5,
            min_reproduction_energy: 70.0,

            mutation_rate_multiplier: 1.0,
            enable_pioneer_reseeding: false, // Честная природа по умолчанию (без читов и магии)
            time_scale: 1.0,
            is_paused: false,
            ticks_per_sec: 40,
        }
    }
}
