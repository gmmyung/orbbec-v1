use orbbec_v1::{Context, Pipeline, PointCloudFilter, Sensor};
use std::time::Duration;

#[test]
#[ignore = "requires exclusive access to one connected legacy Orbbec camera"]
fn calibrated_point_cloud_matches_depth_in_meters() -> Result<(), Box<dyn std::error::Error>> {
    let context = Context::new()?;
    let devices = context.devices()?;
    let device = context.open(devices.first().ok_or("No camera")?)?;
    let mut pipeline = Pipeline::new(&device)?;
    assert!(PointCloudFilter::new(&pipeline).is_err());
    let profiles = pipeline.profiles(Sensor::Depth)?;
    pipeline.start(&[profiles[0]])?;
    {
        let mut filter = PointCloudFilter::new(&pipeline)?;
        let intrinsics = filter.intrinsics();
        eprintln!("Point cloud intrinsics: {intrinsics:?}");
        let mut checked = false;
        for _ in 0..30 {
            let Some(frames) = pipeline.wait(Duration::from_secs(1))? else {
                continue;
            };
            let Some(depth) = frames.depth()? else {
                continue;
            };
            let Some(cloud) = filter.process(&frames)? else {
                continue;
            };
            let samples: Vec<_> = depth.samples()?.collect();
            assert_eq!(cloud.width, depth.video().width()?);
            assert_eq!(cloud.height, depth.video().height()?);
            assert_eq!(cloud.points.len(), samples.len());
            let valid = cloud.points.iter().filter(|p| p[2] > 0.0).count();
            assert!(
                valid > 100,
                "Reference scene should produce many valid points"
            );
            for (i, (point, raw)) in cloud.points.iter().zip(samples).enumerate().step_by(97) {
                if point[2] > 0.0 && raw > 0 {
                    assert!((point[2] - f32::from(raw) * depth.scale_mm()? / 1000.0).abs() < 0.002);
                    let x = (i % cloud.width as usize) as f32;
                    let y = (i / cloud.width as usize) as f32;
                    if x < intrinsics.cx - 20.0 {
                        assert!(point[0] <= 0.0);
                    }
                    if x > intrinsics.cx + 20.0 {
                        assert!(point[0] >= 0.0);
                    }
                    if y < intrinsics.cy - 20.0 {
                        assert!(point[1] <= 0.0);
                    }
                    if y > intrinsics.cy + 20.0 {
                        assert!(point[1] >= 0.0);
                    }
                }
            }
            eprintln!(
                "Point cloud: {}x{}, {valid} valid points",
                cloud.width, cloud.height
            );
            checked = true;
            break;
        }
        assert!(checked, "No calibrated point cloud received");
    }
    pipeline.stop()?;
    Ok(())
}

/// Serial execution only: one physical device cannot be opened by parallel tests.
#[test]
#[ignore = "requires one connected legacy Orbbec camera and USB write permissions"]
fn repeated_capture_and_error_recovery() -> Result<(), Box<dyn std::error::Error>> {
    let context = Context::new()?;
    let descriptors = context.devices()?;
    assert_eq!(descriptors.len(), 1, "Use exactly one camera for this test");
    let device = context.open(&descriptors[0])?;
    let mut pipeline = Pipeline::new(&device)?;
    // The pipeline retains these parents even after application handles go away.
    drop(device);
    drop(context);
    let profiles = pipeline.profiles(Sensor::Depth)?;
    let selected = *profiles
        .iter()
        .find(|p| p.width == 640 && p.fps == 30)
        .ok_or("No 640-wide 30fps depth profile")?;
    assert!(pipeline.wait(Duration::from_millis(1)).is_err());
    assert!(pipeline.start(&[]).is_err());
    let mut invalid = selected;
    invalid.width = 13;
    let error = pipeline
        .start(&[invalid])
        .expect_err("Invalid profile should fail");
    assert!(
        error.function.is_some(),
        "Native error details should survive freeing ob_error"
    );
    for _ in 0..5 {
        pipeline.start(&[selected])?;
        assert!(pipeline.start(&[selected]).is_err());
        assert!(pipeline.wait(Duration::ZERO).is_err());
        let mut count = 0;
        let mut last_index = None;
        for _ in 0..60 {
            let Some(frameset) = pipeline.wait(Duration::from_secs(2))? else {
                continue;
            };
            let Some(depth) = frameset.depth()? else {
                continue;
            };
            let frame = depth.video();
            assert_eq!(frame.width()?, selected.width);
            assert_eq!(frame.height()?, selected.height);
            assert_eq!(
                depth.samples()?.len(),
                (selected.width * selected.height) as usize
            );
            assert!(depth.scale_mm()? > 0.0);
            assert!(depth.distance_mm(selected.width, 0).is_err());
            if let Some(last) = last_index {
                assert!(frame.index()? > last);
            }
            last_index = Some(frame.index()?);
            count += 1;
            if count == 30 {
                break;
            }
        }
        assert_eq!(count, 30);
        pipeline.stop()?;
        pipeline.stop()?;
    }
    // Verify automatic stop on Drop with an active stream.
    pipeline.start(&[selected])?;
    drop(pipeline);
    let context = Context::new()?;
    let device = context.open(&context.devices()?[0])?;
    let mut pipeline = Pipeline::new(&device)?;
    pipeline.start(&[selected])?;
    assert!(pipeline.wait(Duration::from_secs(2))?.is_some());
    pipeline.stop()?;
    Ok(())
}
