use crate::genetics::{
    BRAIN_HIDDEN_SIZE, BRAIN_INPUT_SIZE, BRAIN_OUTPUT_SIZE,
};
use crate::math::{fast_sigmoid, fast_tanh};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Brain {
    pub hidden_state: [f32; BRAIN_HIDDEN_SIZE],
    pub last_inputs: [f32; BRAIN_INPUT_SIZE],
    pub last_outputs: [f32; BRAIN_OUTPUT_SIZE],
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct MotorOutputs {
    pub throttle: f32, // -0.5 to 1.0
    pub steer: f32,    // -1.0 to 1.0 (turn rate)
    pub bite: bool,    // snap jaw / eat
    pub mate: bool,    // attempt courtship
    pub emit_pheromone: f32, // 0.0 to 1.0
}

impl Brain {
    pub fn new() -> Self {
        Brain {
            hidden_state: [0.0; BRAIN_HIDDEN_SIZE],
            last_inputs: [0.0; BRAIN_INPUT_SIZE],
            last_outputs: [0.0; BRAIN_OUTPUT_SIZE],
        }
    }

    pub fn forward(&mut self, inputs: &[f32; BRAIN_INPUT_SIZE], weights: &[f32]) -> MotorOutputs {
        self.last_inputs = *inputs;

        let mut offset = 0;

        // 1. Input -> Hidden (W_ih: BRAIN_INPUT_SIZE x BRAIN_HIDDEN_SIZE)
        let mut hidden_inputs = [0.0f32; BRAIN_HIDDEN_SIZE];
        for h in 0..BRAIN_HIDDEN_SIZE {
            let mut sum = 0.0f32;
            for i in 0..BRAIN_INPUT_SIZE {
                sum += inputs[i] * weights[offset];
                offset += 1;
            }
            hidden_inputs[h] = sum;
        }

        // 2. Hidden -> Hidden Recurrent (W_hh: BRAIN_HIDDEN_SIZE x BRAIN_HIDDEN_SIZE)
        let prev_hidden = self.hidden_state;
        for h in 0..BRAIN_HIDDEN_SIZE {
            let mut sum = 0.0f32;
            for h_prev in 0..BRAIN_HIDDEN_SIZE {
                sum += prev_hidden[h_prev] * weights[offset];
                offset += 1;
            }
            hidden_inputs[h] += sum;
        }

        // 3. Hidden Biases (b_h: BRAIN_HIDDEN_SIZE)
        for h in 0..BRAIN_HIDDEN_SIZE {
            hidden_inputs[h] += weights[offset];
            offset += 1;
        }

        // 4. Update hidden state with leaky integration (CTRNN alpha)
        const ALPHA: f32 = 0.40; // memory inertia
        for h in 0..BRAIN_HIDDEN_SIZE {
            let target_activation = fast_tanh(hidden_inputs[h]);
            self.hidden_state[h] = (1.0 - ALPHA) * self.hidden_state[h] + ALPHA * target_activation;
        }

        // 5. Hidden -> Output (W_ho: BRAIN_HIDDEN_SIZE x BRAIN_OUTPUT_SIZE)
        let mut raw_outputs = [0.0f32; BRAIN_OUTPUT_SIZE];
        for o in 0..BRAIN_OUTPUT_SIZE {
            let mut sum = 0.0f32;
            for h in 0..BRAIN_HIDDEN_SIZE {
                sum += self.hidden_state[h] * weights[offset];
                offset += 1;
            }
            raw_outputs[o] = sum;
        }

        // 6. Output Biases (b_o: BRAIN_OUTPUT_SIZE)
        for o in 0..BRAIN_OUTPUT_SIZE {
            raw_outputs[o] += weights[offset];
            offset += 1;
        }

        self.last_outputs = raw_outputs;

        // Interpret motor actions
        let throttle = fast_tanh(raw_outputs[0]) * 0.75 + 0.25; // bias towards forward exploration [-0.5 .. 1.0]
        let steer = fast_tanh(raw_outputs[1]);                   // [-1.0 .. 1.0]
        let bite = raw_outputs[2] > 0.35;
        let mate = raw_outputs[3] > 0.40;
        let emit_pheromone = fast_sigmoid(raw_outputs[4]);

        MotorOutputs {
            throttle,
            steer,
            bite,
            mate,
            emit_pheromone,
        }
    }
}
