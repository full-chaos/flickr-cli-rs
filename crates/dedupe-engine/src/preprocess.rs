use image::{DynamicImage, GenericImageView};
use ndarray::Array4;

use crate::models::ModelConfig;

/// Preprocess an image for vision model inference.
///
/// Pipeline (same for CLIP and SigLIP2):
/// 1. Resize shortest edge to `config.input_size`, maintaining aspect ratio
/// 2. Center crop to `input_size x input_size`
/// 3. Convert to float32 [0, 1]
/// 4. Normalize with model-specific mean/std
/// 5. Convert to CHW layout with batch dimension: [1, 3, H, W]
pub fn preprocess_image(img: &DynamicImage, config: &ModelConfig) -> Array4<f32> {
    let size = config.input_size;

    // Step 1: Resize so shortest side = input_size
    let (w, h) = img.dimensions();
    let (new_w, new_h) = if w < h {
        (size, (size as f32 * h as f32 / w as f32) as u32)
    } else {
        ((size as f32 * w as f32 / h as f32) as u32, size)
    };

    let resized = img.resize_exact(new_w, new_h, image::imageops::FilterType::Lanczos3);

    // Step 2: Center crop
    let (rw, rh) = resized.dimensions();
    let x_offset = (rw - size) / 2;
    let y_offset = (rh - size) / 2;
    let cropped = resized.crop_imm(x_offset, y_offset, size, size);

    // Steps 3-5: Convert to normalized CHW float tensor
    let rgb = cropped.to_rgb8();
    let s = size as usize;
    let mut tensor = Array4::<f32>::zeros((1, 3, s, s));

    for y in 0..s {
        for x in 0..s {
            let pixel = rgb.get_pixel(x as u32, y as u32);
            for c in 0..3 {
                let val = pixel[c] as f32 / 255.0;
                tensor[[0, c, y, x]] = (val - config.mean[c]) / config.std[c];
            }
        }
    }

    tensor
}

#[cfg(feature = "onnx")]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::SIGLIP2_VIT_B16;
    use image::RgbImage;

    fn make_rgb_image(width: u32, height: u32, r: u8, g: u8, b: u8) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_fn(width, height, |_x, _y| {
            image::Rgb([r, g, b])
        }))
    }

    fn make_gradient_image(width: u32, height: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([(x * 255 / width) as u8, (y * 255 / height) as u8, 128])
        }))
    }

    #[test]
    fn preprocess_square_image_output_shape() {
        let img = make_rgb_image(100, 100, 128, 64, 255);
        let tensor = preprocess_image(&img, &SIGLIP2_VIT_B16);
        let shape = tensor.shape();
        assert_eq!(
            shape,
            &[1, 3, 224, 224],
            "output shape should be [1,3,224,224]"
        );
    }

    #[test]
    fn preprocess_landscape_image_output_shape() {
        // 300x200 — wider than tall
        let img = make_gradient_image(300, 200);
        let tensor = preprocess_image(&img, &SIGLIP2_VIT_B16);
        let shape = tensor.shape();
        assert_eq!(
            shape,
            &[1, 3, 224, 224],
            "landscape image should produce [1,3,224,224]"
        );
    }

    #[test]
    fn preprocess_portrait_image_output_shape() {
        // 200x300 — taller than wide
        let img = make_gradient_image(200, 300);
        let tensor = preprocess_image(&img, &SIGLIP2_VIT_B16);
        let shape = tensor.shape();
        assert_eq!(
            shape,
            &[1, 3, 224, 224],
            "portrait image should produce [1,3,224,224]"
        );
    }

    #[test]
    fn preprocess_pixel_values_normalized_range() {
        // SigLIP2 uses mean=0.5, std=0.5 for all channels.
        // Pixel value 128/255 ≈ 0.502 → (0.502 - 0.5) / 0.5 ≈ 0.004  (near 0)
        // Pixel value 0/255 = 0.0 → (0.0 - 0.5) / 0.5 = -1.0
        // Pixel value 255/255 = 1.0 → (1.0 - 0.5) / 0.5 = 1.0
        // So values should lie in roughly [-1, 1].
        let img = make_rgb_image(100, 100, 128, 64, 255);
        let tensor = preprocess_image(&img, &SIGLIP2_VIT_B16);

        for &val in tensor.iter() {
            assert!(
                val >= -1.5 && val <= 1.5,
                "pixel value {} is outside expected normalized range [-1.5, 1.5]",
                val
            );
        }
    }

    #[test]
    fn preprocess_solid_white_image_values() {
        // white pixel = 255 → (1.0 - 0.5) / 0.5 = 1.0 for all channels
        let img = make_rgb_image(50, 50, 255, 255, 255);
        let tensor = preprocess_image(&img, &SIGLIP2_VIT_B16);

        for &val in tensor.iter() {
            assert!(
                (val - 1.0f32).abs() < 1e-5,
                "white pixel should normalize to 1.0, got {}",
                val
            );
        }
    }

    #[test]
    fn preprocess_solid_black_image_values() {
        // black pixel = 0 → (0.0 - 0.5) / 0.5 = -1.0 for all channels
        let img = make_rgb_image(50, 50, 0, 0, 0);
        let tensor = preprocess_image(&img, &SIGLIP2_VIT_B16);

        for &val in tensor.iter() {
            assert!(
                (val - (-1.0f32)).abs() < 1e-5,
                "black pixel should normalize to -1.0, got {}",
                val
            );
        }
    }
}
