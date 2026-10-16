//! Port extras, not in the original: stored as `settings.cfg` (key = value lines).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hotspots {
    HoldSpace,
    Always,
    Off,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scaling {
    /// Integer scale, sharp pixels.
    Sharp,
    /// Fill the window keeping 4:3, sharp pixels.
    Fill,
    /// Fill, bilinear filtered.
    Smooth,
    /// Fill, scanlines and shadow mask.
    Crt,
    /// Fill, xBRZ 4× pixel-art upscaling.
    Xbrz,
}

impl Scaling {
    pub fn next(self) -> Self {
        match self {
            Scaling::Fill => Scaling::Sharp,
            Scaling::Sharp => Scaling::Smooth,
            Scaling::Smooth => Scaling::Crt,
            Scaling::Crt => Scaling::Xbrz,
            Scaling::Xbrz => Scaling::Fill,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub hotspots: Hotspots,
    pub fullscreen: bool,
    pub scaling: Scaling,
    pub center_cutscenes: bool,
    pub autosave_rooms: bool,
    /// Click an item to take it, click another to combine, click outside the grid to close
    /// holding it; right click closes empty-handed.
    pub modern_inventory: bool,
    /// "cs" = original text, otherwise the code of a `lang-<code>.txt` translation.
    pub language: String,
    /// Speech bubble and label size: 1 = original, 2 = double.
    pub text_scale: u8,
    /// Interpolate walking between the 70 ms game ticks.
    pub smooth_motion: bool,
    /// Blurred picture in the side bars when the window is wider than 4:3.
    pub widescreen: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { hotspots: Hotspots::HoldSpace, fullscreen: false, scaling: Scaling::Fill, center_cutscenes: true, autosave_rooms: true, modern_inventory: true, language: "cs".into(), text_scale: 1, smooth_motion: true, widescreen: true }
    }
}

pub const FILE: &str = "settings.cfg";

impl Settings {
    pub fn parse(text: &str) -> Self {
        let mut s = Self::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            let on = v == "true";
            match k {
                "hotspots" => {
                    s.hotspots = match v {
                        "always" => Hotspots::Always,
                        "off" => Hotspots::Off,
                        _ => Hotspots::HoldSpace,
                    }
                }
                "fullscreen" => s.fullscreen = on,
                "scaling" => {
                    s.scaling = match v {
                        "sharp" => Scaling::Sharp,
                        "smooth" => Scaling::Smooth,
                        "crt" => Scaling::Crt,
                        "xbrz" => Scaling::Xbrz,
                        _ => Scaling::Fill,
                    }
                }
                "center_cutscenes" => s.center_cutscenes = on,
                "autosave_rooms" => s.autosave_rooms = on,
                "modern_inventory" => s.modern_inventory = on,
                "language" => s.language = v.to_string(),
                "smooth_motion" => s.smooth_motion = on,
                "widescreen" => s.widescreen = on,
                "text_scale" => s.text_scale = v.parse().unwrap_or(1).clamp(1, 2),
                _ => {}
            }
        }
        s
    }

    pub fn to_text(&self) -> String {
        format!(
            "hotspots = {}\nfullscreen = {}\nscaling = {}\ncenter_cutscenes = {}\nautosave_rooms = {}\nmodern_inventory = {}\nlanguage = {}\ntext_scale = {}\nsmooth_motion = {}\nwidescreen = {}\n",
            match self.hotspots {
                Hotspots::HoldSpace => "hold-space",
                Hotspots::Always => "always",
                Hotspots::Off => "off",
            },
            self.fullscreen,
            match self.scaling {
                Scaling::Sharp => "sharp",
                Scaling::Fill => "fill",
                Scaling::Smooth => "smooth",
                Scaling::Crt => "crt",
                Scaling::Xbrz => "xbrz",
            },
            self.center_cutscenes,
            self.autosave_rooms,
            self.modern_inventory,
            self.language,
            self.text_scale,
            self.smooth_motion,
            self.widescreen
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let s = Settings { hotspots: Hotspots::Always, fullscreen: true, scaling: Scaling::Sharp, center_cutscenes: true, autosave_rooms: false, modern_inventory: false, language: "en".into(), text_scale: 2, smooth_motion: false, widescreen: false };
        assert_eq!(Settings::parse(&s.to_text()), s);
    }
}
