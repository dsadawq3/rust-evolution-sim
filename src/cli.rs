use std::env;

#[derive(Clone, Debug)]
pub struct CliArgs {
    pub host: String,
    pub port: u16,
    pub initial_creatures: usize,
    pub initial_food: usize,
    pub world_size: f32,
    pub speed: f32,
    pub headless: bool,
    pub benchmark_ticks: u64,
}

impl Default for CliArgs {
    fn default() -> Self {
        CliArgs {
            host: "127.0.0.1".to_string(),
            port: 7890,
            initial_creatures: 160,
            initial_food: 420,
            world_size: 300.0,
            speed: 1.0,
            headless: false,
            benchmark_ticks: 10_000,
        }
    }
}

impl CliArgs {
    pub fn parse() -> Self {
        let mut args = CliArgs::default();
        let raw: Vec<String> = env::args().collect();
        let mut i = 1;

        while i < raw.len() {
            match raw[i].as_str() {
                "--host" => {
                    if i + 1 < raw.len() {
                        args.host = raw[i + 1].clone();
                        i += 1;
                    }
                }
                "--port" | "-p" => {
                    if i + 1 < raw.len() {
                        if let Ok(p) = raw[i + 1].parse::<u16>() {
                            args.port = p;
                        }
                        i += 1;
                    }
                }
                "--creatures" | "-c" => {
                    if i + 1 < raw.len() {
                        if let Ok(c) = raw[i + 1].parse::<usize>() {
                            args.initial_creatures = c;
                        }
                        i += 1;
                    }
                }
                "--food" | "-f" => {
                    if i + 1 < raw.len() {
                        if let Ok(f) = raw[i + 1].parse::<usize>() {
                            args.initial_food = f;
                        }
                        i += 1;
                    }
                }
                "--world-size" | "-w" => {
                    if i + 1 < raw.len() {
                        if let Ok(w) = raw[i + 1].parse::<f32>() {
                            args.world_size = w;
                        }
                        i += 1;
                    }
                }
                "--speed" | "-s" => {
                    if i + 1 < raw.len() {
                        if let Ok(s) = raw[i + 1].parse::<f32>() {
                            args.speed = s;
                        }
                        i += 1;
                    }
                }
                "--headless" => {
                    args.headless = true;
                }
                "--benchmark-ticks" | "-b" => {
                    if i + 1 < raw.len() {
                        if let Ok(b) = raw[i + 1].parse::<u64>() {
                            args.benchmark_ticks = b;
                        }
                        i += 1;
                    }
                }
                "--help" | "-h" => {
                    Self::print_help();
                    std::process::exit(0);
                }
                _ => {}
            }
            i += 1;
        }

        args
    }

    pub fn print_help() {
        println!(
            r#"
AI EVOLUTION ENGINE (RUST + 3D BROWSER FRONTEND)
High-performance Natural Selection & Emergent Genetics Simulation

USAGE:
    evolution_sim [OPTIONS]

OPTIONS:
    -p, --port <PORT>              Web server port (default: 7890)
        --host <HOST>              Bind address (default: 127.0.0.1)
    -c, --creatures <COUNT>        Initial population count (default: 160)
    -f, --food <COUNT>             Initial food count (default: 420)
    -w, --world-size <SIZE>        Square arena size (default: 300.0)
    -s, --speed <SPEED>            Initial time multiplier (default: 1.0)
        --headless                 Run fast benchmark without web server
    -b, --benchmark-ticks <COUNT>  Ticks to run in headless mode (default: 10000)
    -h, --help                     Print help information
"#
        );
    }
}
