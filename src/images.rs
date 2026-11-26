//! In-memory image encoding/decoding for pasted and dropped images.

use std::io::Cursor;

use eframe::egui;
use image::{ImageFormat, RgbaImage};

pub const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;

/// Rejects images whose encoded PNG would not fit the local asset budget.
pub fn check_size(len: usize) -> Result<(), String> {
    if len > MAX_IMAGE_BYTES {
        Err("Imagem muito grande (máx. 10 MB)".to_owned())
    } else {
        Ok(())
    }
}

pub fn encode_png(rgba: &[u8], w: u32, h: u32) -> Result<Vec<u8>, String> {
    let image =
        RgbaImage::from_raw(w, h, rgba.to_vec()).ok_or_else(|| "Dimensões inválidas".to_owned())?;
    let mut out = Cursor::new(Vec::new());
    image
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|err| format!("Não foi possível codificar a imagem: {err}"))?;
    Ok(out.into_inner())
}

/// Re-encodes an egui clipboard image as PNG, unmultiplying its pixels first.
pub fn color_image_to_png(img: &egui::ColorImage) -> Result<(Vec<u8>, u32, u32), String> {
    let width = u32::try_from(img.size[0]).map_err(|_| "Imagem com largura inválida".to_owned())?;
    let height = u32::try_from(img.size[1]).map_err(|_| "Imagem com altura inválida".to_owned())?;
    if width == 0 || height == 0 {
        return Err("Imagem sem dimensões".to_owned());
    }
    let mut rgba = Vec::with_capacity(img.pixels.len() * 4);
    for pixel in &img.pixels {
        rgba.extend_from_slice(&pixel.to_srgba_unmultiplied());
    }
    let png = encode_png(&rgba, width, height)?;
    Ok((png, width, height))
}

pub fn decode(bytes: &[u8]) -> Result<(Vec<u8>, u32, u32), String> {
    let image = image::load_from_memory(bytes).map_err(|err| format!("Imagem inválida: {err}"))?;
    let (width, height) = (image.width(), image.height());
    if width == 0 || height == 0 {
        return Err("Imagem sem dimensões".to_owned());
    }
    Ok((image.into_rgba8().into_raw(), width, height))
}

/// Validates a dropped image file without re-encoding it.
///
/// Rejects oversized buffers before decoding, accepts only PNG/JPEG/GIF/WebP,
/// decodes once to verify integrity and measure dimensions, and returns the
/// original bytes alongside the canonical MIME type.
pub fn prepare_file(bytes: &[u8]) -> Result<(Vec<u8>, String, u32, u32), String> {
    check_size(bytes.len())?;
    let mime = match image::guess_format(bytes) {
        Ok(ImageFormat::Png) => "image/png",
        Ok(ImageFormat::Jpeg) => "image/jpeg",
        Ok(ImageFormat::Gif) => "image/gif",
        Ok(ImageFormat::WebP) => "image/webp",
        _ => {
            return Err("Formato não suportado (use PNG, JPEG, GIF ou WebP)".to_owned());
        }
    };
    let (_, width, height) = decode(bytes)?;
    Ok((bytes.to_vec(), mime.to_owned(), width, height))
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Color32;
    use image::{DynamicImage, Rgb, RgbImage};

    #[test]
    fn size_limit_accepts_max_and_rejects_one_more_byte() {
        assert!(check_size(MAX_IMAGE_BYTES).is_ok());
        assert_eq!(
            check_size(MAX_IMAGE_BYTES + 1).unwrap_err(),
            "Imagem muito grande (máx. 10 MB)"
        );
    }

    #[test]
    fn png_round_trip_keeps_rgba_and_dimensions() {
        let rgba: Vec<u8> = (0..(2 * 3 * 4)).map(|value| value as u8 * 5).collect();
        let png = encode_png(&rgba, 2, 3).unwrap();
        let (decoded, width, height) = decode(&png).unwrap();
        assert_eq!((width, height), (2, 3));
        assert_eq!(decoded, rgba);
    }

    #[test]
    fn encode_rejects_inconsistent_buffer() {
        assert!(encode_png(&[0, 0, 0], 2, 2).is_err());
    }

    #[test]
    fn color_image_round_trip_keeps_dimensions_and_alpha() {
        let image = egui::ColorImage::new(
            [2, 2],
            vec![
                Color32::from_rgba_unmultiplied(255, 0, 0, 128),
                Color32::TRANSPARENT,
                Color32::WHITE,
                Color32::BLACK,
            ],
        );
        let (png, width, height) = color_image_to_png(&image).unwrap();
        assert_eq!((width, height), (2, 2));

        let (rgba, decoded_width, decoded_height) = decode(&png).unwrap();
        assert_eq!((decoded_width, decoded_height), (2, 2));
        assert_eq!(rgba.len(), 2 * 2 * 4);
        assert_eq!(rgba[3], 128, "o alfa semitransparente deve sobreviver");
        assert_eq!(rgba[7], 0, "pixel transparente continua transparente");
    }

    fn encode_sample(format: ImageFormat, w: u32, h: u32) -> Vec<u8> {
        let img = RgbImage::from_fn(w, h, |x, y| Rgb([(x * 40) as u8, (y * 60) as u8, 120]));
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(img)
            .write_to(&mut out, format)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn prepare_file_accepts_png_and_preserves_original_bytes() {
        let rgba: Vec<u8> = (0..(3 * 2 * 4)).map(|v| (v * 9) as u8).collect();
        let png = encode_png(&rgba, 3, 2).unwrap();

        let (out, mime, w, h) = prepare_file(&png).unwrap();

        assert_eq!(mime, "image/png");
        assert_eq!((w, h), (3, 2));
        assert_eq!(out, png);
    }

    #[test]
    fn prepare_file_accepts_jpeg_gif_and_webp_and_preserves_bytes() {
        for (format, expected_mime, w, h) in [
            (ImageFormat::Jpeg, "image/jpeg", 4, 3),
            (ImageFormat::Gif, "image/gif", 5, 2),
            (ImageFormat::WebP, "image/webp", 3, 4),
        ] {
            let encoded = encode_sample(format, w, h);
            let (out, mime, out_w, out_h) = prepare_file(&encoded).unwrap();
            assert_eq!(mime, expected_mime);
            assert_eq!((out_w, out_h), (w, h));
            assert_eq!(out, encoded, "os bytes originais devem ser preservados");
        }
    }

    #[test]
    fn prepare_file_rejects_plain_text() {
        let err = prepare_file(b"isto e um arquivo de texto comum").unwrap_err();
        assert!(
            err.starts_with("Formato não suportado"),
            "erro inesperado: {err}"
        );
    }

    #[test]
    fn prepare_file_rejects_oversized_buffer_before_decoding() {
        let mut oversized = vec![0u8; MAX_IMAGE_BYTES + 1];
        // Cabeçalho PNG seguido de lixo: se decodificasse, falharia com "Imagem inválida".
        oversized[..8].copy_from_slice(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);

        let err = prepare_file(&oversized).unwrap_err();
        assert_eq!(err, "Imagem muito grande (máx. 10 MB)");
    }

    #[test]
    fn prepare_file_rejects_corrupted_png() {
        let corrupted = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 1, 2, 3];
        let err = prepare_file(&corrupted).unwrap_err();
        assert!(err.starts_with("Imagem inválida"), "erro inesperado: {err}");
    }
}
