use crate::creature::Creature;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpeciesInfo {
    pub id: usize,
    pub name: String,
    pub diet: String,
    pub color_hue: f32,
    pub color_hex: String,
    pub count: usize,
    pub avg_jaw: f32,
    pub avg_radius: f32,
    pub avg_speed: f32,
    pub avg_eyes: f32,
    pub avg_armor: f32,
    pub max_generation: usize,
}

pub struct SpeciesAnalyzer;

impl SpeciesAnalyzer {
    pub fn classify_population(creatures: &[Creature]) -> Vec<SpeciesInfo> {
        if creatures.is_empty() {
            return Vec::new();
        }

        // Group creatures by taxonomic trait bucket
        // (Diet: 0=Herbivore, 1=Omnivore, 2=Carnivore) * 100 + (Size: 0=Small, 1=Medium, 2=Large) * 10 + (Eyes: 0,1,2,3+)
        let mut clusters: HashMap<u32, Vec<&Creature>> = HashMap::new();

        for c in creatures {
            if !c.is_alive {
                continue;
            }
            let diet_key = if c.phenotype.jaw_strength < 0.28 {
                0
            } else if c.phenotype.jaw_strength > 0.52 {
                2
            } else {
                1
            };

            let size_key = if c.phenotype.body_radius < 0.95 {
                0
            } else if c.phenotype.body_radius > 1.45 {
                2
            } else {
                1
            };

            let eye_key = c.phenotype.eye_count.min(4) as u32;

            let cluster_id = diet_key * 100 + size_key * 10 + eye_key;
            clusters.entry(cluster_id).or_default().push(c);
        }

        let mut results = Vec::new();
        let mut sorted_clusters: Vec<(u32, Vec<&Creature>)> = clusters.into_iter().collect();
        // Sort by population descending
        sorted_clusters.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

        for (idx, (cluster_id, members)) in sorted_clusters.into_iter().enumerate() {
            let count = members.len();
            if count == 0 {
                continue;
            }

            let mut sum_jaw = 0.0;
            let mut sum_radius = 0.0;
            let mut sum_speed = 0.0;
            let mut sum_eyes = 0.0;
            let mut sum_armor = 0.0;
            let mut sum_hue = 0.0;
            let mut max_gen = 0;

            for m in &members {
                sum_jaw += m.phenotype.jaw_strength;
                sum_radius += m.phenotype.body_radius;
                sum_speed += m.phenotype.base_speed;
                sum_eyes += m.phenotype.eye_count as f32;
                sum_armor += m.phenotype.armor_spikes;
                sum_hue += m.phenotype.color_hue;
                if m.generation > max_gen {
                    max_gen = m.generation;
                }
            }

            let c_f = count as f32;
            let avg_jaw = sum_jaw / c_f;
            let avg_radius = sum_radius / c_f;
            let avg_speed = sum_speed / c_f;
            let avg_eyes = sum_eyes / c_f;
            let avg_armor = sum_armor / c_f;
            let avg_hue = sum_hue / c_f;

            let (diet_label, name_suffix) = if avg_jaw < 0.28 {
                ("Травоядный", "Grazer")
            } else if avg_jaw > 0.52 {
                ("Хищник", "Stalker")
            } else {
                ("Всеядный", "Forager")
            };

            let name_prefix = if avg_radius > 1.4 {
                "Titan"
            } else if avg_radius < 0.95 {
                "Micro"
            } else if avg_speed > 3.8 {
                "Swift"
            } else if avg_armor > 0.35 {
                "Armored"
            } else if avg_eyes >= 4.0 {
                "Multi-Eyed"
            } else {
                "Emerald"
            };

            let species_name = format!("{} {}", name_prefix, name_suffix);
            let color_hex = hsl_to_hex(avg_hue, 0.75, 0.52);

            results.push(SpeciesInfo {
                id: (cluster_id as usize) + (idx * 1000),
                name: species_name,
                diet: diet_label.to_string(),
                color_hue: avg_hue,
                color_hex,
                count,
                avg_jaw,
                avg_radius,
                avg_speed,
                avg_eyes,
                avg_armor,
                max_generation: max_gen,
            });
        }

        results
    }
}

fn hsl_to_hex(h: f32, s: f32, l: f32) -> String {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h_prime = (h * 6.0) % 6.0;
    let x = c * (1.0 - ((h_prime % 2.0) - 1.0).abs());
    let m = l - c * 0.5;

    let (r, g, b) = if h_prime < 1.0 {
        (c, x, 0.0)
    } else if h_prime < 2.0 {
        (x, c, 0.0)
    } else if h_prime < 3.0 {
        (0.0, c, x)
    } else if h_prime < 4.0 {
        (0.0, x, c)
    } else if h_prime < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    let r_i = ((r + m) * 255.0).clamp(0.0, 255.0) as u8;
    let g_i = ((g + m) * 255.0).clamp(0.0, 255.0) as u8;
    let b_i = ((b + m) * 255.0).clamp(0.0, 255.0) as u8;

    format!("#{:02x}{:02x}{:02x}", r_i, g_i, b_i)
}
