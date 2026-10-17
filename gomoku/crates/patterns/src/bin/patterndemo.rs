//! Terminal heatmap demo for the `patterns` threat network.
//!
//! Usage:
//!
//! ```text
//! cargo run -p patterns --bin patterndemo
//! cargo run -p patterns --bin patterndemo -- --map policy --showcase classic-fork
//! cargo run -p patterns --features gpu --bin patterndemo -- --gpu
//! ```
//!
//! The binary renders one of the built-in showcase boards with a selected
//! threat map overlaid as an ANSI-coloured heatmap. With the `gpu` feature
//! enabled, `--gpu` runs the analysis on the `wgpu` backend.

use std::env;
use std::process;

use patterns::net::{ThreatNet, analyze};
use patterns::render::{MapName, ShowcaseName, render_heatmap, showcase_board, showcase_side};

use burn::tensor::Device;

/// Command-line configuration for the demo.
#[derive(Debug, Clone, PartialEq)]
struct Args {
    /// Threat map to render.
    map: MapName,
    /// Built-in position to display.
    showcase: ShowcaseName,
    /// Whether to run on the GPU backend.
    gpu: bool,
}

fn main() {
    let raw: Vec<String> = env::args().skip(1).collect();
    if raw.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return;
    }

    let args = match parse_args(raw.into_iter()) {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("{msg}");
            eprintln!(
                "Usage: patterndemo [OPTIONS]\n\
                 \n\
                 Options:\n\
                 --map <NAME>        Threat map to render (default: double_threes)\n\
                 --showcase <NAME>   Built-in position (default: double-three-fork)\n\
                 --gpu               Run on the wgpu backend (requires `gpu` feature)\n\
                 --help              Print this help message\n\
                 \n\
                 Valid maps: wins, double_threats, double_threes,\n\
                 threes_h, threes_v, threes_dd, threes_du, policy\n\
                 Valid showcases: double-three-fork, classic-fork, corner-fork, quiet"
            );
            process::exit(2);
        }
    };

    if let Err(e) = run(args) {
        eprintln!("patterndemo failed: {e}");
        process::exit(1);
    }
}

fn run(args: Args) -> Result<(), String> {
    let board = showcase_board(args.showcase);
    let side = showcase_side(args.showcase);

    if args.gpu {
        run_gpu(args.showcase, args.map, side, &board)
    } else {
        run_cpu(args.showcase, args.map, side, &board)
    }
}

#[cfg(feature = "gpu")]
fn run_gpu(
    showcase: ShowcaseName,
    map: MapName,
    side: engine::Color,
    board: &engine::Board,
) -> Result<(), String> {
    let device = Device::wgpu(Default::default());
    let net = ThreatNet::new(&device);
    let maps = analyze(&net, board, side, &device);
    print!("{}", render_heatmap(&maps, board, showcase, map, "Wgpu"));
    Ok(())
}

#[cfg(not(feature = "gpu"))]
fn run_gpu(
    _showcase: ShowcaseName,
    _map: MapName,
    _side: engine::Color,
    _board: &engine::Board,
) -> Result<(), String> {
    Err("--gpu requires the `gpu` feature (cargo run -p patterns --features gpu)".to_string())
}

fn run_cpu(
    showcase: ShowcaseName,
    map: MapName,
    side: engine::Color,
    board: &engine::Board,
) -> Result<(), String> {
    let device = Device::flex();
    let net = ThreatNet::new(&device);
    let maps = analyze(&net, board, side, &device);
    print!("{}", render_heatmap(&maps, board, showcase, map, "flex"));
    Ok(())
}

fn parse_args<I>(mut args: I) -> Result<Args, String>
where
    I: Iterator<Item = String>,
{
    let mut map = None;
    let mut showcase = None;
    let mut gpu = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--map" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--map requires a value".to_string())?;
                map = Some(MapName::parse(&value).ok_or_else(|| {
                    format!(
                        "unknown map '{value}'. Valid maps: wins, double_threats, double_threes, \
                         threes_h, threes_v, threes_dd, threes_du, policy"
                    )
                })?);
            }
            "--showcase" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--showcase requires a value".to_string())?;
                showcase = Some(ShowcaseName::parse(&value).ok_or_else(|| {
                    format!(
                        "unknown showcase '{value}'. Valid showcases: \
                         double-three-fork, classic-fork, corner-fork, quiet"
                    )
                })?);
            }
            "--gpu" => gpu = true,
            other => return Err(format!("unknown flag: {other}")),
        }
    }

    Ok(Args {
        map: map.unwrap_or_default(),
        showcase: showcase.unwrap_or_default(),
        gpu,
    })
}

fn print_help() {
    println!("patterndemo — render a Gomoku threat map as a terminal heatmap");
    println!();
    println!("Usage: patterndemo [OPTIONS]");
    println!();
    println!("Options:");
    println!("  --map <NAME>        Threat map to render (default: double_threes)");
    println!("                      wins | double_threats | double_threes |");
    println!("                      threes_h | threes_v | threes_dd | threes_du | policy");
    println!("  --showcase <NAME>   Built-in position (default: double-three-fork)");
    println!("                      double-three-fork | classic-fork | corner-fork | quiet");
    println!("  --gpu               Run on the wgpu backend (requires `gpu` feature)");
    println!("  --help              Print this help message");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_args_rejects_unknown_flag() {
        let result = parse_args(["--nonsense".to_string()].into_iter());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown flag"));
    }

    #[test]
    fn parse_args_accepts_map_and_showcase() {
        let args = parse_args(
            [
                "--map".to_string(),
                "policy".to_string(),
                "--showcase".to_string(),
                "classic-fork".to_string(),
            ]
            .into_iter(),
        )
        .unwrap();

        assert_eq!(args.map, MapName::Policy);
        assert_eq!(args.showcase, ShowcaseName::ClassicFork);
        assert!(!args.gpu);
    }

    #[test]
    fn parse_args_parses_gpu_flag() {
        let args = parse_args(["--gpu".to_string()].into_iter()).unwrap();

        assert!(args.gpu);
        assert_eq!(args.map, MapName::default());
        assert_eq!(args.showcase, ShowcaseName::default());
    }
}
