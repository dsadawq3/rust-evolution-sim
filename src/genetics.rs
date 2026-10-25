use rand::Rng;
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

pub const MAX_EYES: usize = 6;
pub const BRAIN_INPUT_SIZE: usize = 20; // 6 eyes * 2 features + 8 internal sensors
pub const BRAIN_HIDDEN_SIZE: usize = 12;
pub const BRAIN_OUTPUT_SIZE: usize = 5; // throttle, steer, bite, mate, emit_pheromone

pub const BRAIN_WEIGHTS_COUNT: usize = 
    (BRAIN_INPUT_SIZE * BRAIN_HIDDEN_SIZE) + // W_ih
    (BRAIN_HIDDEN_SIZE * BRAIN_HIDDEN_SIZE) + // W_hh (recurrent)
    (BRAIN_HIDDEN_SIZE * BRAIN_OUTPUT_SIZE) + // W_ho
    BRAIN_HIDDEN_SIZE + // b_h
    BRAIN_OUTPUT_SIZE;  // b_o

pub const CORE_GENES_COUNT: usize = 14;
pub const EYE_GENES_PER_EYE: usize = 4; // azimuth, elevation, range, fov
pub const EYE_GENES_TOTAL: usize = MAX_EYES * EYE_GENES_PER_EYE;

pub const TOTAL_CHROMOSOME_LEN: usize = CORE_GENES_COUNT + EYE_GENES_TOTAL + BRAIN_WEIGHTS_COUNT;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EyePhenotype {
    pub azimuth: f32,   // -PI to PI
    pub elevation: f32, // 0.1 to 0.8
    pub range: f32,     // 10.0 to 70.0
    pub fov: f32,       // 0.3 to 2.2 radians
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Phenotype {
    pub body_radius: f32,
    pub mass: f32,
    pub base_speed: f32,
    pub max_health: f32,
    pub metabolic_efficiency: f32,
    pub jaw_strength: f32, // 0.0 = pure herbivore, 1.0 = hyper carnivore
    pub armor_spikes: f32,
    pub lifespan: f32,
    pub maturity_age_ratio: f32,
    pub base_choosiness: f32,
    pub clutch_size: usize,
    pub energy_investment: f32,
    pub color_hue: f32,
    pub color_saturation: f32,
    pub eye_count: usize,
    pub eyes: Vec<EyePhenotype>,

    // Flattened brain weights
    pub brain_weights: Vec<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Chromosome {
    pub alleles: Vec<f32>,
}

impl Chromosome {
    pub fn random_init<R: Rng>(rng: &mut R) -> Self {
        let mut alleles = Vec::with_capacity(TOTAL_CHROMOSOME_LEN);

        // Core genes initialization (normalized or natural scale)
        // 0: body_radius [0.7 .. 1.8]
        alleles.push(rng.gen_range(0.8..1.5));
        // 1: base_speed [1.5 .. 5.0]
        alleles.push(rng.gen_range(2.0..4.0));
        // 2: max_health [40.0 .. 150.0]
        alleles.push(rng.gen_range(60.0..120.0));
        // 3: metabolic_efficiency [0.7 .. 1.5]
        alleles.push(rng.gen_range(0.9..1.3));
        // 4: jaw_strength [0.0 .. 0.5] (mostly peaceful initially, can mutate to carnivorous)
        alleles.push(rng.gen_range(0.05..0.35));
        // 5: armor_spikes [0.0 .. 0.5]
        alleles.push(rng.gen_range(0.0..0.25));
        // 6: lifespan [2500 .. 7000]
        alleles.push(rng.gen_range(3000.0..6000.0));
        // 7: maturity_age_ratio [0.1 .. 0.3]
        alleles.push(rng.gen_range(0.12..0.25));
        // 8: base_choosiness [0.1 .. 0.8]
        alleles.push(rng.gen_range(0.2..0.6));
        // 9: clutch_size [1.0 .. 2.8]
        alleles.push(rng.gen_range(1.0..2.2));
        // 10: energy_investment [0.25 .. 0.45]
        alleles.push(rng.gen_range(0.30..0.45));
        // 11: color_hue (around 0.42 = mint/green)
        alleles.push(rng.gen_range(0.38..0.46));
        // 12: color_saturation
        alleles.push(rng.gen_range(0.6..0.85));
        // 13: eye_count [1.0 .. 4.5]
        alleles.push(rng.gen_range(1.8..3.2));

        // Eye genes: 6 eyes * 4 traits
        for i in 0..MAX_EYES {
            // Symmetrical or spread azimuths initially
            let default_azi = match i {
                0 => 0.35,
                1 => -0.35,
                2 => 1.2,
                3 => -1.2,
                4 => 2.5,
                _ => -2.5,
            };
            alleles.push(default_azi + rng.gen_range(-0.2..0.2)); // azimuth
            alleles.push(rng.gen_range(0.25..0.65));               // elevation
            alleles.push(rng.gen_range(25.0..55.0));               // range
            alleles.push(rng.gen_range(0.7..1.6));                 // fov
        }

        // Brain weights: random small values
        for _ in 0..BRAIN_WEIGHTS_COUNT {
            alleles.push(rng.gen_range(-0.6..0.6));
        }

        Chromosome { alleles }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiploidGenome {
    pub maternal: Chromosome,
    pub paternal: Chromosome,
    pub parent_imprint_hue: f32,
    pub parent_imprint_radius: f32,
}

impl DiploidGenome {
    pub fn random_new<R: Rng>(rng: &mut R) -> Self {
        let maternal = Chromosome::random_init(rng);
        let paternal = Chromosome::random_init(rng);
        let imprint_hue = (maternal.alleles[11] + paternal.alleles[11]) * 0.5;
        let imprint_radius = (maternal.alleles[0] + paternal.alleles[0]) * 0.5;
        DiploidGenome {
            maternal,
            paternal,
            parent_imprint_hue: imprint_hue,
            parent_imprint_radius: imprint_radius,
        }
    }

    pub fn express_phenotype(&self) -> Phenotype {
        let m = &self.maternal.alleles;
        let p = &self.paternal.alleles;

        // Helper: codominant average
        let avg = |idx: usize| (m[idx] + p[idx]) * 0.5;

        let body_radius = avg(0).clamp(0.5, 2.5);
        let mass = body_radius * body_radius * 1.8;
        let base_speed = avg(1).clamp(0.8, 6.5);
        let max_health = avg(2).clamp(30.0, 250.0);
        let metabolic_efficiency = avg(3).clamp(0.5, 2.2);
        let jaw_strength = avg(4).clamp(0.0, 1.0);
        let armor_spikes = avg(5).clamp(0.0, 1.0);
        let lifespan = avg(6).clamp(1500.0, 15000.0);
        let maturity_age_ratio = avg(7).clamp(0.05, 0.45);
        let base_choosiness = avg(8).clamp(0.0, 1.0);
        let clutch_size = avg(9).round().clamp(1.0, 4.0) as usize;
        let energy_investment = avg(10).clamp(0.18, 0.55);
        let color_hue = avg(11).rem_euclid(1.0);
        let color_saturation = avg(12).clamp(0.2, 1.0);
        let eye_count = avg(13).round().clamp(1.0, MAX_EYES as f32) as usize;

        let mut eyes = Vec::with_capacity(eye_count);
        let eye_offset = CORE_GENES_COUNT;
        for i in 0..eye_count {
            let base = eye_offset + i * EYE_GENES_PER_EYE;
            let azimuth = avg(base).clamp(-PI, PI);
            let elevation = avg(base + 1).clamp(0.1, 0.85);
            let range = avg(base + 2).clamp(10.0, 75.0);
            let fov = avg(base + 3).clamp(0.25, 2.4);
            eyes.push(EyePhenotype {
                azimuth,
                elevation,
                range,
                fov,
            });
        }

        // Brain weights: codominant blend
        let brain_offset = CORE_GENES_COUNT + EYE_GENES_TOTAL;
        let mut brain_weights = Vec::with_capacity(BRAIN_WEIGHTS_COUNT);
        for i in 0..BRAIN_WEIGHTS_COUNT {
            brain_weights.push(avg(brain_offset + i));
        }

        Phenotype {
            body_radius,
            mass,
            base_speed,
            max_health,
            metabolic_efficiency,
            jaw_strength,
            armor_spikes,
            lifespan,
            maturity_age_ratio,
            base_choosiness,
            clutch_size,
            energy_investment,
            color_hue,
            color_saturation,
            eye_count,
            eyes,
            brain_weights,
        }
    }

    // Sexual reproduction with crossover and mutation
    pub fn crossover_and_mutate<R: Rng>(
        mother: &DiploidGenome,
        father: &DiploidGenome,
        mutation_rate_mult: f32,
        rng: &mut R,
    ) -> Self {
        let maternal_gamete = make_gamete(&mother.maternal, &mother.paternal, mutation_rate_mult, rng);
        let paternal_gamete = make_gamete(&father.maternal, &father.paternal, mutation_rate_mult, rng);

        // Imprint on maternal and paternal phenotypes
        let mother_pheno = mother.express_phenotype();
        let father_pheno = father.express_phenotype();
        let imprint_hue = (mother_pheno.color_hue + father_pheno.color_hue) * 0.5;
        let imprint_radius = (mother_pheno.body_radius + father_pheno.body_radius) * 0.5;

        DiploidGenome {
            maternal: maternal_gamete,
            paternal: paternal_gamete,
            parent_imprint_hue: imprint_hue,
            parent_imprint_radius: imprint_radius,
        }
    }
}

// Meiosis: Crossover between two homologous chromosomes + Point mutations
fn make_gamete<R: Rng>(
    c1: &Chromosome,
    c2: &Chromosome,
    mutation_mult: f32,
    rng: &mut R,
) -> Chromosome {
    let len = c1.alleles.len();
    let mut gamete = Vec::with_capacity(len);

    // Pick 1 to 3 crossover points
    let num_crossovers = rng.gen_range(1..=3);
    let mut crossover_points = Vec::with_capacity(num_crossovers);
    for _ in 0..num_crossovers {
        crossover_points.push(rng.gen_range(0..len));
    }
    crossover_points.sort_unstable();

    let mut use_c1 = rng.gen_bool(0.5);
    let mut next_cp_idx = 0;

    let base_mut_chance = 0.04 * mutation_mult;

    for i in 0..len {
        if next_cp_idx < crossover_points.len() && i == crossover_points[next_cp_idx] {
            use_c1 = !use_c1;
            next_cp_idx += 1;
        }

        let mut val = if use_c1 { c1.alleles[i] } else { c2.alleles[i] };

        // Mutation check
        if rng.gen_bool(base_mut_chance.min(0.8) as f64) {
            // Gaussian-like perturbation
            let delta = rng.gen_range(-1.0..1.0) * rng.gen_range(-1.0..1.0);
            let scale = if i < CORE_GENES_COUNT {
                match i {
                    0 => 0.15, // radius
                    1 => 0.35, // speed
                    2 => 8.0,  // health
                    3 => 0.1,  // metabolism
                    4 => 0.08, // jaw strength
                    5 => 0.08, // armor
                    6 => 250.0,// lifespan
                    11 => 0.05,// hue
                    13 => 0.4, // eye count
                    _ => 0.1,
                }
            } else if i < CORE_GENES_COUNT + EYE_GENES_TOTAL {
                0.25 // eye placement & range
            } else {
                0.35 // brain synaptic weight
            };

            val += delta * scale;
        }

        gamete.push(val);
    }

    Chromosome { alleles: gamete }
}
