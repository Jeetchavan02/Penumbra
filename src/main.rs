//! Command-line front end: parse arguments, render, write a PNG.

use std::path::Path;
use std::process::ExitCode;
use std::str::FromStr;

use penumbra::accel::SplitMethod;
use penumbra::mesh::load_obj;
use penumbra::render::{render, Lighting, RenderSettings};
use penumbra::scene::library::{self, Finish};

const USAGE: &str = "\
penumbra: a CPU path tracer

USAGE:
    penumbra [OPTIONS]

OPTIONS:
    --scene <NAME>     Scene to render [default: penumbra]
    --list             List the available scenes and exit
    --obj <PATH>       Render a Wavefront OBJ file in a studio instead of a named scene
    --finish <NAME>    Surface for --obj: clay, gold or glass [default: clay]
    --turn <DEGREES>   Rotate the --obj model about the vertical axis [default: 0]
    --width <PX>       Image width; height follows the scene's aspect ratio [default: 800]
    --spp <N>          Samples per pixel [default: 128]
    --depth <N>        Maximum path length in bounces [default: 50]
    --seed <N>         Random seed; the same seed gives the same image [default: 0]
    --out <PATH>       Output PNG [default: <scene or model name>.png]
    --bvh <METHOD>     BVH split method: sah or midpoint [default: sah]
    --lighting <MODE>  How lights are found: mis, light or bsdf [default: mis]
    --no-direct        Same as --lighting bsdf (plain path tracing)
    --clamp <VALUE>    Clamp each sample's radiance to remove fireflies (biased)
    --threads <N>      Worker threads [default: all cores]
    --quiet            No progress output
    -h, --help         Show this help
";

struct Options {
    scene: String,
    obj: Option<String>,
    finish: Finish,
    turn: f64,
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
        obj: None,
        finish: Finish::default(),
        turn: 0.0,
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
            "--obj" => options.obj = Some(parse_value(&flag, args.next())?),
            "--turn" => options.turn = parse_value(&flag, args.next())?,
            "--finish" => {
                let name: String = parse_value(&flag, args.next())?;
                options.finish = Finish::from_name(&name)
                    .ok_or_else(|| format!("unknown finish: {name} (expected one of {})", Finish::NAMES.join(", ")))?;
            }
            "--out" => options.out = Some(parse_value(&flag, args.next())?),
            "--width" => options.settings.width = parse_value(&flag, args.next())?,
            "--spp" => options.settings.samples_per_pixel = parse_value(&flag, args.next())?,
            "--depth" => options.settings.max_depth = parse_value(&flag, args.next())?,
            "--seed" => options.settings.seed = parse_value(&flag, args.next())?,
            "--clamp" => options.settings.max_sample_radiance = Some(parse_value(&flag, args.next())?),
            "--threads" => options.threads = Some(parse_value(&flag, args.next())?),
            "--no-direct" => options.settings.lighting = Lighting::BsdfOnly,
            "--lighting" => {
                let name: String = parse_value(&flag, args.next())?;
                options.settings.lighting = Lighting::from_name(&name).ok_or_else(|| {
                    format!("unknown lighting mode: {name} (expected one of {})", Lighting::NAMES.join(", "))
                })?;
            }
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
    if matches!(s.max_sample_radiance, Some(limit) if limit.is_nan() || limit <= 0.0) {
        return Err("--clamp must be positive".to_string());
    }
    if options.threads == Some(0) {
        return Err("--threads must be at least 1".to_string());
    }
    Ok(Command::Render(options))
}

fn run(mut options: Options) -> Result<(), String> {
    let (name, scene) = match &options.obj {
        Some(path) => {
            let mesh = load_obj(path).map_err(|e| format!("{path}: {e}"))?;
            if mesh.triangle_count() == 0 {
                return Err(format!("{path}: no faces found"));
            }
            let name = Path::new(path).file_stem().map_or("model".into(), |stem| stem.to_string_lossy().into_owned());
            (name, library::studio(&mesh, options.finish, options.turn).build(options.split))
        }
        None => {
            let scene = library::build(&options.scene, options.split)
                .ok_or_else(|| format!("unknown scene: {} (try --list)", options.scene))?;
            (options.scene.clone(), scene)
        }
    };
    options.settings.height = scene.camera.height_for(options.settings.width);
    let settings = &options.settings;

    if let Some(threads) = options.threads {
        rayon::ThreadPoolBuilder::new().num_threads(threads).build_global().map_err(|e| e.to_string())?;
    }

    let bvh = scene.bvh();
    if settings.progress {
        eprintln!(
            "{}: {} primitives, {} lights, BVH {:?} with {} nodes (depth {})",
            name,
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

    let out = options.out.unwrap_or_else(|| format!("{name}.png"));
    film.save_png(&out).map_err(|e| format!("could not write {out}: {e}"))?;

    let seconds = stats.elapsed.as_secs_f64();
    let samples = settings.width as f64 * settings.height as f64 * settings.samples_per_pixel as f64;
    eprintln!("wrote {out} in {seconds:.2}s ({:.2}M samples/s)", samples / seconds / 1e6);
    if stats.rejected_samples > 0 {
        eprintln!("warning: {} samples were NaN, infinite or negative and were discarded", stats.rejected_samples);
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
