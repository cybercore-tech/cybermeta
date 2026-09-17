//! Tag listing helpers and name ↔ ExifTag mapping for common edits.

use anyhow::{bail, Result};
use little_exif::exif_tag::ExifTag;
use little_exif::ifd::ExifTagGroup;
use little_exif::metadata::Metadata;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagGroup {
    Image,
    Exif,
    Gps,
    Interop,
    Other,
}

impl TagGroup {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Image => "Image",
            Self::Exif => "EXIF",
            Self::Gps => "GPS",
            Self::Interop => "Interop",
            Self::Other => "Other",
        }
    }

    pub fn from_exif(group: ExifTagGroup) -> Self {
        match group {
            ExifTagGroup::GENERIC => Self::Image,
            ExifTagGroup::EXIF => Self::Exif,
            ExifTagGroup::GPS => Self::Gps,
            ExifTagGroup::INTEROP => Self::Interop,
        }
    }
}

impl From<TagGroup> for ExifTagGroup {
    fn from(value: TagGroup) -> Self {
        match value {
            TagGroup::Image => ExifTagGroup::GENERIC,
            TagGroup::Exif => ExifTagGroup::EXIF,
            TagGroup::Gps => ExifTagGroup::GPS,
            TagGroup::Interop => ExifTagGroup::INTEROP,
            TagGroup::Other => ExifTagGroup::GENERIC,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TagView {
    pub name: String,
    pub hex: u16,
    pub group: TagGroup,
    pub value: String,
}

pub fn collect_tags(metadata: &Metadata) -> Vec<TagView> {
    let mut out = Vec::new();
    for tag in metadata {
        // Skip IFD/data offset pointers — not useful in the browser.
        match tag.get_tag_type() {
            little_exif::exif_tag::TagType::VALUE => {}
            _ => continue,
        }
        let (name, value) = split_debug(tag);
        out.push(TagView {
            name,
            hex: tag.as_u16(),
            group: TagGroup::from_exif(tag.get_group()),
            value,
        });
    }
    out.sort_by(|a, b| {
        a.group
            .as_str()
            .cmp(b.group.as_str())
            .then(a.name.cmp(&b.name))
    });
    out
}

fn split_debug(tag: &ExifTag) -> (String, String) {
    let dbg = format!("{tag:?}");
    // Variants look like `Make("Canon")` or `Orientation([1])` or
    // `UnknownSTRING("x", 123, GPS)`.
    let Some(paren) = dbg.find('(') else {
        return (dbg, String::new());
    };
    let name = dbg[..paren].to_string();
    let inner = dbg[paren + 1..].trim_end_matches(')');
    let value = if name.starts_with("Unknown") {
        // Take first field only for unknowns.
        inner
            .split(',')
            .next()
            .unwrap_or(inner)
            .trim()
            .trim_matches('"')
            .to_string()
    } else {
        pretty_value(inner)
    };
    (name, value)
}

fn pretty_value(inner: &str) -> String {
    let trimmed = inner.trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        return trimmed[1..trimmed.len() - 1].to_string();
    }
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        return trimmed[1..trimmed.len() - 1].trim().to_string();
    }
    trimmed.to_string()
}

pub fn parse_set_assignment(assignment: &str) -> Result<ExifTag> {
    let Some((name, value)) = assignment.split_once('=') else {
        bail!("expected TAG=VALUE, got `{assignment}`");
    };
    string_tag_by_name(name.trim(), value.trim())
}

pub fn string_tag_by_name(name: &str, value: &str) -> Result<ExifTag> {
    let key = name.trim();
    let v = value.to_string();
    Ok(match key.to_ascii_lowercase().as_str() {
        "imagedescription" | "image_description" | "description" => ExifTag::ImageDescription(v),
        "make" => ExifTag::Make(v),
        "model" => ExifTag::Model(v),
        "software" => ExifTag::Software(v),
        "artist" => ExifTag::Artist(v),
        "copyright" => ExifTag::Copyright(v),
        "datetime" | "date_time" | "modifydate" | "modify_date" => ExifTag::ModifyDate(v),
        "datetimeoriginal" | "date_time_original" => ExifTag::DateTimeOriginal(v),
        "datetimedigitized" | "date_time_digitized" | "createdate" | "create_date" => {
            ExifTag::CreateDate(v)
        }
        "lensmake" | "lens_make" => ExifTag::LensMake(v),
        "lensmodel" | "lens_model" => ExifTag::LensModel(v),
        "gpslatituderef" | "gps_latitude_ref" => ExifTag::GPSLatitudeRef(v),
        "gpslongituderef" | "gps_longitude_ref" => ExifTag::GPSLongitudeRef(v),
        other => bail!(
            "unsupported editable tag `{other}` — try Make, Model, Software, Artist, Copyright, DateTime, ImageDescription, LensMake, LensModel"
        ),
    })
}

pub fn lookup_tag_hex_group(name: &str) -> Result<(u16, ExifTagGroup)> {
    let tag = string_tag_by_name(name, "")?;
    Ok((tag.as_u16(), tag.get_group()))
}
