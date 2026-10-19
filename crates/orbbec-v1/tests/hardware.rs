use orbbec_v1::{Context, Pipeline, Sensor};
use std::time::Duration;

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
