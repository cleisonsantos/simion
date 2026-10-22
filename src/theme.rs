//! Visual identity: fonts, colors and egui style.

use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Shadow,
    Stroke, TextStyle, ThemePreference, Vec2, Visuals, style::Selection,
};

pub const FONT_BODY: &str = "body";
pub const FONT_MEDIUM: &str = "medium";
pub const FONT_SEMIBOLD: &str = "semibold";
pub const FONT_SEMIBOLD_ITALIC: &str = "semibold-italic";
pub const FONT_ITALIC: &str = "italic";
pub const FONT_MONO: &str = "mono";

pub const GUTTER: f32 = 34.0;
pub const MAX_CONTENT_WIDTH: f32 = 756.0;
pub const INDENT_WIDTH: f32 = 22.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Light,
    Dark,
}

impl Mode {
    pub fn toggled(self) -> Self {
        match self {
            Self::Light => Self::Dark,
            Self::Dark => Self::Light,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub fn from_str(value: &str) -> Self {
        if value == "dark" {
            Self::Dark
        } else {
            Self::Light
        }
    }
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub bg: Color32,
    pub panel: Color32,
    pub elevated: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub faint: Color32,
    pub hover: Color32,
    pub active: Color32,
    pub border: Color32,
    pub accent: Color32,
    pub accent_text: Color32,
    pub accent_soft: Color32,
    pub selection: Color32,
    pub code_bg: Color32,
    pub callout_bg: Color32,
    pub quote_bar: Color32,
    pub danger: Color32,
    pub shadow: Color32,
    pub toast_bg: Color32,
    pub toast_text: Color32,
}

pub fn palette(mode: Mode) -> Palette {
    match mode {
        Mode::Light => Palette {
            dark: false,
            bg: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            panel: Color32::from_rgb(0xF7, 0xF7, 0xF5),
            elevated: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            text: Color32::from_rgb(0x37, 0x35, 0x2F),
            muted: Color32::from_rgb(0x78, 0x77, 0x74),
            faint: Color32::from_rgb(0x9B, 0x9A, 0x97),
            hover: Color32::from_rgb(0xEF, 0xEE, 0xEB),
            active: Color32::from_rgb(0xE6, 0xE5, 0xE1),
            border: Color32::from_rgb(0xEB, 0xEA, 0xE7),
            accent: Color32::from_rgb(0x23, 0x83, 0xE2),
            accent_text: Color32::from_rgb(0x1F, 0x74, 0xC9),
            accent_soft: Color32::from_rgba_unmultiplied(0x23, 0x83, 0xE2, 0x1C),
            selection: Color32::from_rgba_unmultiplied(0x23, 0x83, 0xE2, 0x33),
            code_bg: Color32::from_rgb(0xF4, 0xF4, 0xF2),
            callout_bg: Color32::from_rgb(0xF7, 0xF7, 0xF5),
            quote_bar: Color32::from_rgb(0x37, 0x35, 0x2F),
            danger: Color32::from_rgb(0xE0, 0x3E, 0x3E),
            shadow: Color32::from_black_alpha(28),
            toast_bg: Color32::from_rgb(0x2F, 0x2E, 0x2B),
            toast_text: Color32::from_rgb(0xF7, 0xF7, 0xF5),
        },
        Mode::Dark => Palette {
            dark: true,
            bg: Color32::from_rgb(0x19, 0x19, 0x19),
            panel: Color32::from_rgb(0x20, 0x20, 0x20),
            elevated: Color32::from_rgb(0x25, 0x25, 0x24),
            text: Color32::from_rgb(0xE6, 0xE6, 0xE4),
            muted: Color32::from_rgb(0x9A, 0x9A, 0x98),
            faint: Color32::from_rgb(0x6E, 0x6E, 0x6C),
            hover: Color32::from_rgb(0x2B, 0x2B, 0x2A),
            active: Color32::from_rgb(0x34, 0x34, 0x33),
            border: Color32::from_rgb(0x2E, 0x2E, 0x2D),
            accent: Color32::from_rgb(0x5A, 0xA9, 0xF5),
            accent_text: Color32::from_rgb(0x74, 0xB7, 0xF7),
            accent_soft: Color32::from_rgba_unmultiplied(0x5A, 0xA9, 0xF5, 0x24),
            selection: Color32::from_rgba_unmultiplied(0x5A, 0xA9, 0xF5, 0x40),
            code_bg: Color32::from_rgb(0x24, 0x24, 0x23),
            callout_bg: Color32::from_rgb(0x23, 0x23, 0x22),
            quote_bar: Color32::from_rgb(0x4C, 0x4C, 0x4A),
            danger: Color32::from_rgb(0xF0, 0x6A, 0x6A),
            shadow: Color32::from_black_alpha(90),
            toast_bg: Color32::from_rgb(0xEC, 0xEC, 0xEA),
            toast_text: Color32::from_rgb(0x1D, 0x1D, 0x1C),
        },
    }
}

pub fn family(name: &str) -> FontFamily {
    FontFamily::Name(name.into())
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    let add = |fonts: &mut FontDefinitions, name: &str, bytes: &'static [u8]| {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    };

    add(
        &mut fonts,
        "inter",
        include_bytes!("../assets/fonts/Inter-Regular.ttf"),
    );
    add(
        &mut fonts,
        "inter-medium",
        include_bytes!("../assets/fonts/Inter-Medium.ttf"),
    );
    add(
        &mut fonts,
        "inter-semibold",
        include_bytes!("../assets/fonts/Inter-SemiBold.ttf"),
    );
    add(
        &mut fonts,
        "inter-italic",
        include_bytes!("../assets/fonts/Inter-Italic.ttf"),
    );
    add(
        &mut fonts,
        "inter-semibold-italic",
        include_bytes!("../assets/fonts/Inter-SemiBoldItalic.ttf"),
    );
    add(
        &mut fonts,
        "jetbrains-mono",
        include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf"),
    );
    fonts.font_data.insert(
        "noto-emoji".to_owned(),
        Arc::new(
            FontData::from_static(include_bytes!("../assets/fonts/NotoEmoji-Regular.ttf")).tweak(
                egui::FontTweak {
                    scale: 0.82,
                    ..Default::default()
                },
            ),
        ),
    );
    fonts.font_data.insert(
        "emoji-icon".to_owned(),
        Arc::new(
            FontData::from_static(include_bytes!("../assets/fonts/emoji-icon-font.ttf")).tweak(
                egui::FontTweak {
                    scale: 0.9,
                    ..Default::default()
                },
            ),
        ),
    );

    // Fallbacks, in priority order. The rendered glyphs inherit the text color,
    // which keeps page icons consistent with the theme.
    let fallback = [
        "noto-emoji",
        "emoji-icon",
        "egui-icons",
        "Ubuntu-Light",
        "Hack",
    ];
    let make_family = |primary: &[&str]| {
        primary
            .iter()
            .chain(fallback.iter())
            .map(|name| (*name).to_owned())
            .collect::<Vec<String>>()
    };

    fonts.families.insert(
        FontFamily::Proportional,
        make_family(&["inter", "Ubuntu-Light"]),
    );
    fonts.families.insert(
        FontFamily::Monospace,
        make_family(&["jetbrains-mono", "Hack"]),
    );
    fonts
        .families
        .insert(family(FONT_BODY), make_family(&["inter", "Ubuntu-Light"]));
    fonts
        .families
        .insert(family(FONT_MEDIUM), make_family(&["inter-medium", "inter"]));
    fonts.families.insert(
        family(FONT_SEMIBOLD),
        make_family(&["inter-semibold", "inter-medium"]),
    );
    fonts
        .families
        .insert(family(FONT_ITALIC), make_family(&["inter-italic", "inter"]));
    fonts.families.insert(
        family(FONT_SEMIBOLD_ITALIC),
        make_family(&["inter-semibold-italic", "inter-semibold", "inter-italic"]),
    );
    fonts
        .families
        .insert(family(FONT_MONO), make_family(&["jetbrains-mono", "Hack"]));

    ctx.set_fonts(fonts);
}

pub fn apply(ctx: &egui::Context, palette: &Palette, zoom: f32) {
    ctx.set_theme(if palette.dark {
        ThemePreference::Dark
    } else {
        ThemePreference::Light
    });

    let palette = *palette;
    ctx.all_styles_mut(move |style| {
        let mut visuals = if palette.dark {
            Visuals::dark()
        } else {
            Visuals::light()
        };

        visuals.override_text_color = Some(palette.text);
        visuals.weak_text_color = Some(palette.muted);
        visuals.panel_fill = palette.bg;
        visuals.window_fill = palette.elevated;
        visuals.extreme_bg_color = palette.code_bg;
        visuals.faint_bg_color = palette.callout_bg;
        visuals.code_bg_color = palette.code_bg;
        visuals.hyperlink_color = palette.accent;
        visuals.selection = Selection {
            bg_fill: palette.selection,
            stroke: Stroke::new(1.0, palette.text),
        };
        visuals.text_cursor.stroke = Stroke::new(2.0, palette.accent_text);
        visuals.window_corner_radius = CornerRadius::same(12);
        visuals.menu_corner_radius = CornerRadius::same(10);
        visuals.window_stroke = Stroke::new(1.0, palette.border);
        visuals.window_shadow = Shadow {
            offset: [0, 12],
            blur: 32,
            spread: 0,
            color: palette.shadow,
        };
        visuals.popup_shadow = Shadow {
            offset: [0, 6],
            blur: 20,
            spread: 0,
            color: palette.shadow,
        };
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, palette.border);
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, palette.text);
        visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        visuals.widgets.inactive.bg_fill = palette.hover;
        visuals.widgets.inactive.bg_stroke = Stroke::NONE;
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, palette.text);
        visuals.widgets.hovered.weak_bg_fill = palette.hover;
        visuals.widgets.hovered.bg_fill = palette.hover;
        visuals.widgets.hovered.bg_stroke = Stroke::NONE;
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, palette.text);
        visuals.widgets.active.weak_bg_fill = palette.active;
        visuals.widgets.active.bg_fill = palette.active;
        visuals.widgets.active.bg_stroke = Stroke::NONE;
        visuals.widgets.active.fg_stroke = Stroke::new(1.0, palette.text);
        visuals.widgets.open.weak_bg_fill = palette.active;

        style.visuals = visuals;

        let z = zoom;
        style
            .text_styles
            .insert(TextStyle::Body, FontId::new(15.5 * z, family(FONT_BODY)));
        style.text_styles.insert(
            TextStyle::Button,
            FontId::new(14.5 * z, family(FONT_MEDIUM)),
        );
        style
            .text_styles
            .insert(TextStyle::Small, FontId::new(12.5 * z, family(FONT_BODY)));
        style.text_styles.insert(
            TextStyle::Heading,
            FontId::new(24.0 * z, family(FONT_SEMIBOLD)),
        );
        style.text_styles.insert(
            TextStyle::Monospace,
            FontId::new(13.5 * z, family(FONT_MONO)),
        );

        style.spacing.item_spacing = Vec2::new(8.0, 4.0);
        style.spacing.button_padding = Vec2::new(10.0, 5.0);
        style.spacing.interact_size = Vec2::new(30.0, 30.0);
        style.spacing.menu_margin = Margin::same(6);
        style.spacing.window_margin = Margin::same(14);
        style.spacing.indent = 18.0;
        style.spacing.scroll.floating = true;
        style.spacing.scroll.bar_width = 7.0;
        style.spacing.scroll.bar_inner_margin = 3.0;
        style.spacing.scroll.bar_outer_margin = 3.0;
    });
}
