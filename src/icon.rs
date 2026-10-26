//! Icone da janela do Simion.
//!
//! O icone da marca e carregado do PNG oficial (256x256, RGBA) embutido no
//! binario, em vez de redesenhado em runtime numa resolucao fixa de 128x128.
//! Era essa resolucao unica que fazia o Windows reduzir para 16x16/32x32 na
//! barra de tarefas sem mipmap, produzindo serrilhado e halo no alpha (#2).
//!
//! `png` ja vem na arvore de dependencias (transitiva do `eframe`/`image`), e
//! aqui filtramos exatamente para ele, sem trazer decodificadores extras.

use eframe::egui::IconData;
use png::{ColorType, Decoder, Transformations};

/// PNG oficial da marca, embutido no binario.
const APP_ICON_PNG: &[u8] = include_bytes!("../assets/branding/simion-256.png");

pub fn app_icon() -> IconData {
    match decode_png(APP_ICON_PNG) {
        Ok(icon) => icon,
        Err(err) => {
            // Nunca abortar por causa do icone: sem icone o app ainda abre.
            eprintln!("Simion: nao foi possivel decodificar o icone ({err}); usando fallback");
            fallback_icon()
        }
    }
}

fn decode_png(bytes: &[u8]) -> Result<IconData, String> {
    let mut decoder = Decoder::new(std::io::Cursor::new(bytes));
    // Expande paleta/cinza/16-bit para RGBA8 e aplica o `tRNS` se existir.
    decoder.set_transformations(Transformations::normalize_to_color8() | Transformations::ALPHA);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buffer = vec![0u8; reader.output_buffer_size().unwrap_or(0)];
    let info = reader.next_frame(&mut buffer).map_err(|e| e.to_string())?;
    if info.color_type != ColorType::Rgba {
        return Err(format!(
            "PNG nao normalizado para RGBA (recebido {:?})",
            info.color_type
        ));
    }
    buffer.truncate(info.buffer_size());
    Ok(IconData {
        rgba: buffer,
        width: info.width,
        height: info.height,
    })
}

/// Icone de emergencia (quadrado verde), usado so se o PNG falhar.
fn fallback_icon() -> IconData {
    const SIZE: u32 = 64;
    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.copy_from_slice(&[0x0A, 0x5C, 0x4A, 0xFF]);
    }
    IconData {
        rgba,
        width: SIZE,
        height: SIZE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodifica_o_png_da_marca_em_256() {
        let icon = app_icon();
        assert_eq!(icon.width, 256, "o icone deve sair na resolucao da marca");
        assert_eq!(icon.height, 256);
        assert_eq!(icon.rgba.len(), 256 * 256 * 4);
    }

    #[test]
    fn canto_transparente_e_centro_opaco() {
        let icon = app_icon();
        let corner = 0; // canto superior esquerdo do quadrado arredondado
        assert_eq!(icon.rgba[corner + 3], 0, "canto deveria ser transparente");
        let center = ((128 * 256) + 128) * 4;
        assert_eq!(icon.rgba[center + 3], 255, "centro deveria ser opaco");
    }

    #[test]
    fn fallback_nunca_fica_vazio() {
        let icon = fallback_icon();
        assert_eq!(icon.rgba.len(), (64 * 64 * 4) as usize);
        assert_eq!(icon.rgba[3], 255);
    }
}
