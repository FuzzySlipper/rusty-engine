//! Star catalogues: the stars a star field (`StarsDescriptor`) draws. A
//! product's content holds one as `*.stars.json`; the Engine admits it into
//! a packed binary resource the renderer reads.
//!
//! The content format is JSON: `{"format": "rusty.stars.v1", "stars": [[x,
//! y, z, magnitude, r, g, b], ...]}`. `x, y, z` is toward the star in the
//! catalogue's frame (any length), `magnitude` astronomical (smaller is
//! brighter) and `r, g, b` its linear colour.

use serde::Deserialize;

/// The content format's name.
pub const STAR_CATALOGUE_FORMAT: &str = "rusty.stars.v1";
/// The most stars a catalogue may hold.
pub const MAX_STARS: usize = 65_536;
/// Magnitudes run within this either side of 0.
pub const MAX_STAR_MAGNITUDE: f32 = 30.0;
/// The largest colour channel.
pub const MAX_STAR_COLOR: f32 = 16.0;

/// The packed resource's leading bytes, then a little-endian `u32` count and
/// seven little-endian `f32`s a star: unit direction, magnitude, colour.
const PACKED_MAGIC: &[u8; 8] = b"RSTARS01";
const STAR_FLOATS: usize = 7;

/// One star: toward it (unit length) in the catalogue's frame, its
/// magnitude and its linear colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Star {
    pub direction: [f32; 3],
    pub magnitude: f32,
    pub color: [f32; 3],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StarCatalogueError {
    /// Not the JSON format, or the wrong `format` name.
    Format(String),
    /// Star `index` has a zero or non-finite direction, a magnitude outside
    /// `±MAX_STAR_MAGNITUDE` or a colour outside `0..=MAX_STAR_COLOR`.
    Star(usize),
    /// More than `MAX_STARS`, or none.
    Count(usize),
    /// A packed resource that is not one.
    Packed,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogueJson {
    format: String,
    stars: Vec<[f32; STAR_FLOATS]>,
}

/// Parse and check a catalogue's content, directions made unit length.
pub fn parse_star_catalogue(bytes: &[u8]) -> Result<Vec<Star>, StarCatalogueError> {
    let json: CatalogueJson = serde_json::from_slice(bytes)
        .map_err(|error| StarCatalogueError::Format(error.to_string()))?;
    if json.format != STAR_CATALOGUE_FORMAT {
        return Err(StarCatalogueError::Format(format!(
            "format is `{}`, not `{STAR_CATALOGUE_FORMAT}`",
            json.format
        )));
    }
    if json.stars.is_empty() || json.stars.len() > MAX_STARS {
        return Err(StarCatalogueError::Count(json.stars.len()));
    }
    json.stars
        .iter()
        .enumerate()
        .map(|(index, [x, y, z, magnitude, r, g, b])| {
            let length = (x * x + y * y + z * z).sqrt();
            let color = [*r, *g, *b];
            let valid = length.is_finite()
                && length > 0.0
                && magnitude.is_finite()
                && magnitude.abs() <= MAX_STAR_MAGNITUDE
                && color
                    .iter()
                    .all(|channel| (0.0..=MAX_STAR_COLOR).contains(channel));
            valid
                .then_some(Star {
                    direction: [x / length, y / length, z / length],
                    magnitude: *magnitude,
                    color,
                })
                .ok_or(StarCatalogueError::Star(index))
        })
        .collect()
}

/// The packed resource of checked stars.
pub fn pack_star_catalogue(stars: &[Star]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(12 + stars.len() * STAR_FLOATS * 4);
    bytes.extend_from_slice(PACKED_MAGIC);
    bytes.extend_from_slice(&(stars.len() as u32).to_le_bytes());
    for star in stars {
        for value in star
            .direction
            .iter()
            .chain([star.magnitude].iter())
            .chain(star.color.iter())
        {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    bytes
}

/// The stars of a packed resource.
pub fn unpack_star_catalogue(bytes: &[u8]) -> Result<Vec<Star>, StarCatalogueError> {
    let rest = bytes
        .strip_prefix(PACKED_MAGIC.as_slice())
        .ok_or(StarCatalogueError::Packed)?;
    let (count, rest) = rest.split_at_checked(4).ok_or(StarCatalogueError::Packed)?;
    let count = u32::from_le_bytes(count.try_into().expect("four bytes")) as usize;
    if count > MAX_STARS || rest.len() != count * STAR_FLOATS * 4 {
        return Err(StarCatalogueError::Packed);
    }
    Ok(rest
        .as_chunks::<{ STAR_FLOATS * 4 }>()
        .0
        .iter()
        .map(|star| {
            let value = |index: usize| {
                f32::from_le_bytes(star[index * 4..][..4].try_into().expect("four bytes"))
            };
            Star {
                direction: [value(0), value(1), value(2)],
                magnitude: value(3),
                color: [value(4), value(5), value(6)],
            }
        })
        .collect())
}

/// A star field drawn in the sky: the stars of the catalogue resource
/// `catalogue` (`pack_star_catalogue`), turned into the world by
/// `orientation` (a unit quaternion, x y z w), those brighter than
/// `limiting_magnitude` shown, rolling off over half a magnitude, scaled by
/// `brightness` and twinkling by `twinkle` (0 to 1); and the texture `band`
/// (2:1 equirectangular in the catalogue's frame, the Milky Way) added at
/// `band_strength` (0 to 1).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StarsDescriptor {
    pub catalogue: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub band: Option<String>,
    pub orientation: [f32; 4],
    pub limiting_magnitude: f32,
    pub brightness: f32,
    pub band_strength: f32,
    pub twinkle: f32,
}

impl StarsDescriptor {
    /// The largest brightness a product may set.
    pub const MAX_BRIGHTNESS: f32 = 1000.0;

    /// A catalogue named, a finite orientation with some length, a finite
    /// limiting magnitude within `±MAX_STAR_MAGNITUDE`, a brightness within
    /// `0..=MAX_BRIGHTNESS`, and a band strength and twinkle within `0..=1`.
    pub fn valid(&self) -> bool {
        let unit = 0.0..=1.0;
        let length_squared: f32 = self.orientation.iter().map(|value| value * value).sum();
        !self.catalogue.is_empty()
            && self.band.as_ref().is_none_or(|band| !band.is_empty())
            && self.orientation.iter().all(|value| value.is_finite())
            && length_squared > 0.0
            && self.limiting_magnitude.is_finite()
            && self.limiting_magnitude.abs() <= MAX_STAR_MAGNITUDE
            && (0.0..=Self::MAX_BRIGHTNESS).contains(&self.brightness)
            && unit.contains(&self.band_strength)
            && unit.contains(&self.twinkle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_catalogue_parses_packs_and_unpacks() {
        let json = br#"{"format": "rusty.stars.v1", "stars": [[0, 2, 0, 1.5, 1, 0.9, 0.8], [3, 0, 4, -1, 0.7, 0.8, 1]]}"#;
        let stars = parse_star_catalogue(json).expect("valid");
        assert_eq!(stars[0].direction, [0.0, 1.0, 0.0]);
        assert_eq!(stars[1].direction, [0.6, 0.0, 0.8]);
        assert_eq!(
            unpack_star_catalogue(&pack_star_catalogue(&stars)),
            Ok(stars)
        );
    }

    #[test]
    fn bad_catalogues_are_refused() {
        let refused = |json: &str| parse_star_catalogue(json.as_bytes()).is_err();
        assert!(refused(
            r#"{"format": "other", "stars": [[0, 1, 0, 1, 1, 1, 1]]}"#
        ));
        assert!(refused(r#"{"format": "rusty.stars.v1", "stars": []}"#));
        assert!(refused(
            r#"{"format": "rusty.stars.v1", "stars": [[0, 0, 0, 1, 1, 1, 1]]}"#
        ));
        assert!(refused(
            r#"{"format": "rusty.stars.v1", "stars": [[0, 1, 0, 99, 1, 1, 1]]}"#
        ));
        assert!(refused(
            r#"{"format": "rusty.stars.v1", "stars": [[0, 1, 0, 1, -1, 1, 1]]}"#
        ));
        assert_eq!(
            unpack_star_catalogue(b"RSTARS01\x02\0\0\0"),
            Err(StarCatalogueError::Packed)
        );
    }
}
