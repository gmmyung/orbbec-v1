use crate::camera::Plane;
use eframe::egui::{Color32, ColorImage};

/// Invalid depth stays black; near points are warm and far points are cool.
pub fn depth_image(depth: &Plane<u16>, scale_mm: f32, near_m: f32, far_m: f32) -> ColorImage {
    let pixels = depth
        .pixels
        .iter()
        .map(|&raw| {
            if raw == 0 {
                return Color32::BLACK;
            }
            let t =
                ((f32::from(raw) * scale_mm / 1000.0 - near_m) / (far_m - near_m)).clamp(0.0, 1.0);
            let stops = [
                [244.0, 76.0, 68.0],
                [255.0, 206.0, 80.0],
                [56.0, 208.0, 153.0],
                [43.0, 182.0, 219.0],
                [66.0, 91.0, 210.0],
            ];
            let position = t * 4.0;
            let index = (position as usize).min(3);
            let mix = position - index as f32;
            let rgb: [u8; 3] = std::array::from_fn(|i| {
                (stops[index][i] * (1.0 - mix) + stops[index + 1][i] * mix) as u8
            });
            Color32::from_rgb(rgb[0], rgb[1], rgb[2])
        })
        .collect();
    ColorImage::new(depth.size, pixels)
}

pub fn infrared_image(plane: &Plane<u16>) -> ColorImage {
    let min = plane.pixels.iter().copied().min().unwrap_or(0);
    let max = plane.pixels.iter().copied().max().unwrap_or(0);
    let range = u32::from(max.saturating_sub(min)).max(1);
    let pixels = plane
        .pixels
        .iter()
        .map(|&raw| {
            let gray = (u32::from(raw.saturating_sub(min)) * 255 / range) as u8;
            Color32::from_gray(gray)
        })
        .collect();
    ColorImage::new(plane.size, pixels)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn depth_preserves_invalid_pixels_and_clamps_range() {
        let image = depth_image(
            &Plane {
                size: [5, 1],
                pixels: vec![0, 100, 500, 3000, 65535],
            },
            1.0,
            0.5,
            3.0,
        );
        assert_eq!(image.pixels[0], Color32::BLACK);
        assert_eq!(image.pixels[1], image.pixels[2]);
        assert_eq!(image.pixels[3], image.pixels[4]);
        assert!(image.pixels[2].r() > image.pixels[2].b());
        assert!(image.pixels[3].b() > image.pixels[3].r());
    }
    #[test]
    fn infrared_normalizes_without_dividing_by_zero() {
        let image = infrared_image(&Plane {
            size: [2, 1],
            pixels: vec![100, 1100],
        });
        assert_eq!(image.pixels, [Color32::BLACK, Color32::WHITE]);
        let image = infrared_image(&Plane {
            size: [1, 1],
            pixels: vec![0],
        });
        assert_eq!(image.pixels, [Color32::BLACK]);
    }
}
