//! In-memory image encoding/decoding for pasted clipboard images.

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

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Color32;

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
}
