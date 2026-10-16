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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub hotspots: Hotspots,
    pub fullscreen: bool,
    pub scaling: Scaling,
    pub center_cutscenes: bool,
    pub autosave_rooms: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { hotspots: Hotspots::HoldSpace, fullscreen: false, scaling: Scaling::Fill, center_cutscenes: false, autosave_rooms: true }
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
                "scaling" => s.scaling = if v == "sharp" { Scaling::Sharp } else { Scaling::Fill },
                "center_cutscenes" => s.center_cutscenes = on,
                "autosave_rooms" => s.autosave_rooms = on,
                _ => {}
            }
        }
        s
    }

    pub fn to_text(&self) -> String {
        format!(
            "hotspots = {}\nfullscreen = {}\nscaling = {}\ncenter_cutscenes = {}\nautosave_rooms = {}\n",
            match self.hotspots {
                Hotspots::HoldSpace => "hold-space",
                Hotspots::Always => "always",
                Hotspots::Off => "off",
            },
            self.fullscreen,
            if self.scaling == Scaling::Sharp { "sharp" } else { "fill" },
            self.center_cutscenes,
            self.autosave_rooms
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let s = Settings { hotspots: Hotspots::Always, fullscreen: true, scaling: Scaling::Sharp, center_cutscenes: true, autosave_rooms: false };
        assert_eq!(Settings::parse(&s.to_text()), s);
    }
}
