//! Command-line front end: parse arguments, render, write a PNG.

use std::process::ExitCode;
use std::str::FromStr;

use penumbra::accel::SplitMethod;
use penumbra::render::{render, RenderSettings};
use penumbra::scene::library;

const USAGE: &str = "\
penumbra: a CPU path tracer

USAGE:
    penumbra [OPTIONS]

OPTIONS:
    --scene <NAME>     Scene to render [default: penumbra]
    --list             List the available scenes and exit
    --width <PX>       Image width; height follows the scene's aspect ratio [default: 800]
    --spp <N>          Samples per pixel [default: 128]
    --depth <N>        Maximum path length in bounces [default: 50]
    --seed <N>         Random seed; the same seed gives the same image [default: 0]
    --out <PATH>       Output PNG [default: <scene>.png]
    --bvh <METHOD>     BVH split method: sah or midpoint [default: sah]
    --no-direct        Disable direct light sampling (plain path tracing)
    --clamp <VALUE>    Clamp each sample's radiance to remove fireflies (biased)
    --threads <N>      Worker threads [default: all cores]
    --quiet            No progress output
    -h, --help         Show this help
";

struct Options {
    scene: String,
    out: Option<String>,
    split: SplitMethod,
    threads: Option<usize>,
    settings: RenderSettings,
}

enum Command {
    Render(Options),
    List,
    Help,
}

fn parse_value<T: FromStr>(flag: &str, value: Option<String>) -> Result<T, String> {
    let value = value.ok_or_else(|| format!("{flag} needs a value"))?;
    value.parse().map_err(|_| format!("invalid value for {flag}: {value}"))
}

fn parse(mut args: impl Iterator<Item = String>) -> Result<Command, String> {
    let mut options = Options {
        scene: "penumbra".to_string(),
        out: None,
        split: SplitMethod::Sah,
        threads: None,
        settings: RenderSettings { width: 800, samples_per_pixel: 128, progress: true, ..RenderSettings::default() },
    };

    while let Some(flag) = args.next() {
        match flag.as_str() {
            "-h" | "--help" => return Ok(Command::Help),
            "--list" => return Ok(Command::List),
            "--scene" => options.scene = parse_value(&flag, args.next())?,
            "--out" => options.out = Some(parse_value(&flag, args.next())?),
            "--width" => options.settings.width = parse_value(&flag, args.next())?,
            "--spp" => options.settings.samples_per_pixel = parse_value(&flag, args.next())?,
            "--depth" => options.settings.max_depth = parse_value(&flag, args.next())?,
            "--seed" => options.settings.seed = parse_value(&flag, args.next())?,
            "--clamp" => options.settings.max_sample_radiance = Some(parse_value(&flag, args.next())?),
            "--threads" => options.threads = Some(parse_value(&flag, args.next())?),
            "--no-direct" => options.settings.direct_lighting = false,
            "--quiet" => options.settings.progress = false,
            "--bvh" => {
                options.split = match parse_value::<String>(&flag, args.next())?.as_str() {
                    "sah" => SplitMethod::Sah,
                    "midpoint" => SplitMethod::Midpoint,
                    other => return Err(format!("unknown BVH method: {other} (expected sah or midpoint)")),
                }
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    let s = &options.settings;
    if s.width == 0 || s.samples_per_pixel == 0 || s.max_depth == 0 {
        return Err("--width, --spp and --depth must be at least 1".to_string());
    }
    if matches!(s.max_sample_radiance, Some(limit) if !(limit > 0.0)) {
        return Err("--clamp must be positive".to_string());
    }
    if options.threads == Some(0) {
        return Err("--threads must be at least 1".to_string());
    }
    Ok(Command::Render(options))
}

fn run(mut options: Options) -> Result<(), String> {
    let scene = library::build(&options.scene, options.split)
        .ok_or_else(|| format!("unknown scene: {} (try --list)", options.scene))?;
    options.settings.height = scene.camera.height_for(options.settings.width);
    let settings = &options.settings;

    if let Some(threads) = options.threads {
        rayon::ThreadPoolBuilder::new().num_threads(threads).build_global().map_err(|e| e.to_string())?;
    }

    let bvh = scene.bvh();
    if settings.progress {
        eprintln!(
            "{}: {} primitives, {} lights, BVH {:?} with {} nodes (depth {})",
            options.scene,
            bvh.primitives().len(),
            scene.lights().len(),
            options.split,
            bvh.node_count(),
            bvh.depth()
        );
        eprintln!(
            "{}x{}, {} spp, depth {}, {} threads",
            settings.width,
            settings.height,
            settings.samples_per_pixel,
            settings.max_depth,
            rayon::current_num_threads()
        );
    }

    let (film, stats) = render(&scene, settings);

    let out = options.out.unwrap_or_else(|| format!("{}.png", options.scene));
    film.save_png(&out).map_err(|e| format!("could not write {out}: {e}"))?;

    let seconds = stats.elapsed.as_secs_f64();
    let samples = settings.width as f64 * settings.height as f64 * settings.samples_per_pixel as f64;
    eprintln!("wrote {out} in {seconds:.2}s ({:.2}M samples/s)", samples / seconds / 1e6);
    if stats.rejected_samples > 0 {
        eprintln!(
            "warning: {} samples were NaN, infinite or negative and were discarded",
            stats.rejected_samples
        );
    }
    Ok(())
}

fn main() -> ExitCode {
    let result = match parse(std::env::args().skip(1)) {
        Ok(Command::Help) => {
            print!("{USAGE}");
            Ok(())
        }
        Ok(Command::List) => {
            for entry in library::SCENES {
                println!("{:10} {}", entry.name, entry.description);
            }
            Ok(())
        }
        Ok(Command::Render(options)) => run(options),
        Err(message) => Err(format!("{message}\n\n{USAGE}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}
