use orbbec_v1::{Context, Pipeline, Sensor, sdk_version};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("SDK {}", sdk_version());
    let context = Context::new()?;
    let devices = context.devices()?;
    println!("{} device(s)", devices.len());
    for descriptor in &devices {
        println!("Device: {descriptor:?}");
        let device = context.open(descriptor)?;
        println!("Info: {:?}", device.info()?);
        let pipeline = Pipeline::new(&device)?;
        for sensor in [Sensor::Depth, Sensor::Color, Sensor::Infrared] {
            match pipeline.profiles(sensor) {
                Ok(profiles) => {
                    for (index, profile) in profiles.iter().enumerate() {
                        println!("  [{index:2}] {profile}");
                    }
                }
                Err(error) => println!("  {sensor:?} unavailable: {error}"),
            }
        }
    }
    Ok(())
}
