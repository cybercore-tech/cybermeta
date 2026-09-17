//! Named scrub / spoof profiles.

use super::MetaDoc;
use anyhow::Result;
use little_exif::exif_tag::ExifTag;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    StripGps,
    AnonymousCamera,
    ClearAll,
}

impl Preset {
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "strip-gps" | "strip_gps" | "gps" => Some(Self::StripGps),
            "anonymous-camera" | "anonymous_camera" | "anon" => Some(Self::AnonymousCamera),
            "clear-all" | "clear_all" | "clear" | "all" => Some(Self::ClearAll),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::StripGps => "strip-gps",
            Self::AnonymousCamera => "anonymous-camera",
            Self::ClearAll => "clear-all",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::StripGps => "Remove GPS IFD and geolocation tags",
            Self::AnonymousCamera => "Strip device identity; set generic Make/Model/Software",
            Self::ClearAll => "Reduce to structural TIFF tags only (no EXIF/GPS identity)",
        }
    }
}

pub fn list_presets() -> &'static [Preset] {
    &[
        Preset::StripGps,
        Preset::AnonymousCamera,
        Preset::ClearAll,
    ]
}

pub fn apply_preset(doc: &mut MetaDoc, preset: Preset) -> Result<()> {
    match preset {
        Preset::StripGps => {
            doc.strip_gps();
        }
        Preset::AnonymousCamera => {
            doc.strip_gps();
            for name in [
                "Make",
                "Model",
                "Software",
                "Artist",
                "Copyright",
                "LensMake",
                "LensModel",
                "ImageDescription",
            ] {
                let _ = doc.remove_named_tag(name);
            }
            // Serial / body IDs often arrive as unknown tags — wipe MakerNote.
            doc.metadata.remove_tag(ExifTag::MakerNote(Vec::new()));
            doc.metadata.remove_tag(ExifTag::UserComment(Vec::new()));
            doc.metadata.set_tag(ExifTag::Make("Generic".into()));
            doc.metadata.set_tag(ExifTag::Model("Camera".into()));
            doc.metadata
                .set_tag(ExifTag::Software("cybermeta".into()));
        }
        Preset::ClearAll => {
            doc.strip_all();
        }
    }
    Ok(())
}
