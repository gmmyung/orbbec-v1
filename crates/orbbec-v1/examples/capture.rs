use orbbec_v1::{Context, Format, Pipeline, Sensor, VideoProfile, sdk_version};
use std::{
    error::Error,
    fs,
    io::Write,
    path::PathBuf,
    time::{Duration, Instant},
};

struct Args {
    frames: usize,
    cycles: usize,
    color: bool,
    ir: bool,
    serial: Option<String>,
    depth_profile: Option<usize>,
    output: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = Args {
        frames: 30,
        cycles: 1,
        color: false,
        ir: false,
        serial: None,
        depth_profile: None,
        output: None,
    };
    let mut cli = std::env::args().skip(1);
    while let Some(flag) = cli.next() {
        match flag.as_str() {
            "--frames" => args.frames = cli.next().ok_or("--frames requires a value")?.parse()?,
            "--cycles" => args.cycles = cli.next().ok_or("--cycles requires a value")?.parse()?,
            "--color" => args.color = true,
            "--ir" => args.ir = true,
            "--serial" => args.serial = Some(cli.next().ok_or("--serial requires a value")?),
            "--depth-profile" => {
                args.depth_profile = Some(
                    cli.next()
                        .ok_or("--depth-profile requires an index")?
                        .parse()?,
                )
            }
            "--output" => {
                args.output = Some(cli.next().ok_or("--output requires a PGM path")?.into())
            }
            "--help" | "-h" => {
                println!(
                    "capture [--frames N] [--cycles N] [--color] [--ir] [--serial SERIAL] [--depth-profile INDEX] [--output FILE.pgm]"
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {flag}").into()),
        }
    }
    if args.frames == 0 || args.cycles == 0 {
        return Err("frames and cycles must be positive".into());
    }
    let context = Context::new()?;
    let devices = context.devices()?;
    let device = if let Some(serial) = &args.serial {
        context.open_by_serial(serial)?
    } else {
        if devices.len() > 1 {
            return Err("Multiple cameras connected; select --serial".into());
        }
        let descriptor = devices
            .first()
            .ok_or("No camera found; check USB permissions")?;
        context.open(descriptor)?
    };
    let info = device.info()?;
    println!(
        "SDK {} camera={} firmware={} uid={}",
        sdk_version(),
        info.name,
        info.firmware,
        info.uid
    );
    let mut pipeline = Pipeline::new(&device)?;
    let depth_profiles = pipeline.profiles(Sensor::Depth)?;
    let depth = if let Some(index) = args.depth_profile {
        *depth_profiles
            .get(index)
            .ok_or("Depth profile index out of range; run enumerate")?
    } else {
        preferred(&depth_profiles, None)?
    };
    let mut profiles = vec![depth];
    if args.color {
        profiles.push(preferred(
            &pipeline.profiles(Sensor::Color)?,
            Some(Format::RGB),
        )?);
    }
    if args.ir {
        profiles.push(preferred(&pipeline.profiles(Sensor::Infrared)?, None)?);
    }
    for profile in &profiles {
        println!("Enable {profile}");
    }
    let mut last_depth = None;
    for cycle in 0..args.cycles {
        pipeline.start(&profiles)?;
        let start = Instant::now();
        let mut count = 0;
        let mut attempts = 0;
        while count < args.frames {
            attempts += 1;
            if attempts > args.frames.saturating_mul(4).saturating_add(10)
                || start.elapsed() > Duration::from_secs(60)
            {
                return Err(
                    format!("Captured only {count}/{} complete framesets", args.frames).into(),
                );
            }
            let Some(frameset) = pipeline.wait(Duration::from_secs(2))? else {
                continue;
            };
            let Some(depth) = frameset.depth()? else {
                continue;
            };
            let color = if args.color {
                Some(frameset.color()?.ok_or("Frameset is missing color")?)
            } else {
                None
            };
            let ir = if args.ir {
                Some(frameset.infrared()?.ok_or("Frameset is missing infrared")?)
            } else {
                None
            };
            let video = depth.video();
            let width = video.width()?;
            let height = video.height()?;
            let samples: Vec<u16> = depth.samples()?.collect();
            let valid = samples.iter().filter(|&&v| v != 0).count();
            let min = samples.iter().copied().filter(|&v| v != 0).min();
            let max = samples.iter().copied().max();
            if count == 0 || count + 1 == args.frames {
                println!(
                    "cycle={} frame={} index={} depth={}x{} {} bytes={} timestamp_us={} scale_mm={} valid={}/{} raw_range={:?}..{:?} center_mm={:?}",
                    cycle + 1,
                    count + 1,
                    video.index()?,
                    width,
                    height,
                    video.format()?,
                    video.data()?.len(),
                    video.timestamp_us()?,
                    depth.scale_mm()?,
                    valid,
                    samples.len(),
                    min,
                    max,
                    depth.distance_mm(width / 2, height / 2)?
                );
                for (name, frame) in [("color", color.as_ref()), ("IR", ir.as_ref())] {
                    if let Some(frame) = frame {
                        println!(
                            "  {name}={}x{} {} bytes={} timestamp_us={}",
                            frame.width()?,
                            frame.height()?,
                            frame.format()?,
                            frame.data()?.len(),
                            frame.timestamp_us()?
                        );
                    }
                }
            }
            if args.output.is_some() && count + 1 == args.frames {
                last_depth = Some((width, height, samples));
            }
            count += 1;
        }
        pipeline.stop()?;
        println!(
            "cycle={} captured={} elapsed_s={:.3}; stopped",
            cycle + 1,
            count,
            start.elapsed().as_secs_f32()
        );
    }
    if let (Some(path), Some((width, height, samples))) = (args.output, last_depth) {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        let mut file = std::io::BufWriter::new(fs::File::create(&path)?);
        write!(file, "P5\n{width} {height}\n65535\n")?;
        for sample in samples {
            file.write_all(&sample.to_be_bytes())?;
        }
        file.flush()?;
        println!("Saved raw depth PGM: {}", path.display());
    }
    Ok(())
}

fn preferred(
    profiles: &[VideoProfile],
    format: Option<Format>,
) -> Result<VideoProfile, Box<dyn Error>> {
    profiles
        .iter()
        .copied()
        .find(|p| p.width == 640 && p.fps == 30 && format.is_none_or(|f| p.format == f))
        .or_else(|| {
            profiles
                .iter()
                .copied()
                .find(|p| p.width == 640 && p.fps == 30)
        })
        .or_else(|| profiles.first().copied())
        .ok_or_else(|| "Sensor has no video profiles".into())
}
