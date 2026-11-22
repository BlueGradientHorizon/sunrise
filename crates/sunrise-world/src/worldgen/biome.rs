//! Biome definitions and multi-noise biome resolution over an imported parameter table.
use super::{
    DecodeError,
    biome_temperature::BiomeTemperatureSettings,
    climate::{
        ClimateIndex, ClimateSampler, ClimateSearch, ClimateVolume, IndexError, Parameter,
        ParameterPoint,
    },
    density::identifier,
};
use crate::environment_attributes::{AttributeMap, packed_rgb};
use serde_json::Value;
use sunrise_core::density::{SampleError, SamplerContext};

/// The admitted biome file, retaining the fields needed by the generation and join readers.
/// `Biome.DIRECT_CODEC` reads climate, positional attributes, special effects and generation
/// settings. `Biome.NETWORK_CODEC` reads the same climate and effects, without the generation
/// holder sets. Those sets are resolved by the generator against its worldgen registries.
pub struct BiomeDefinition(Value);

impl BiomeDefinition {
    pub fn decode(mut value: Value) -> Result<Self, DecodeError> {
        BiomeTemperatureSettings::from_json(&value.to_string())?;
        let record = value.as_object_mut().ok_or(DecodeError::Field("biome"))?;
        if let Some(attributes) = record.get("attributes") {
            let attributes = AttributeMap::parse(attributes).map_err(DecodeError::Registry)?;
            // EnvironmentAttributes' two `notPositional` attributes, refused by
            // EnvironmentAttributeMap.CODEC_ONLY_POSITIONAL.
            if let Some(attribute) = attributes
                .attributes()
                .find(|a| matches!(a.id, "gameplay/sky_light_level" | "gameplay/fast_lava"))
            {
                return Err(DecodeError::Registry(format!(
                    "attribute {} cannot be positional",
                    attribute.id
                )));
            }
        }
        let effects = record
            .get_mut("effects")
            .and_then(Value::as_object_mut)
            .ok_or(DecodeError::Field("effects"))?;
        // BiomeSpecialEffects.CODEC and ExtraCodecs.STRING_RGB_COLOR. Canonical hex
        // carries the decoded value to the renderer and the network, including vector inputs.
        for (key, field, required) in [
            ("water_color", "effects.water_color", true),
            ("foliage_color", "effects.foliage_color", false),
            ("dry_foliage_color", "effects.dry_foliage_color", false),
            ("grass_color", "effects.grass_color", false),
        ] {
            match effects.get_mut(key) {
                Some(color) => {
                    let rgb = packed_rgb(color).ok_or(DecodeError::Field(field))?;
                    *color = Value::String(format!("#{rgb:06x}"));
                }
                None if required => return Err(DecodeError::Field(field)),
                None => {}
            }
        }
        if let Some(modifier) = effects.get("grass_color_modifier")
            && !matches!(modifier.as_str(), Some("none" | "dark_forest" | "swamp"))
        {
            return Err(DecodeError::Field("effects.grass_color_modifier"));
        }
        // BiomeGenerationSettings.CODEC requires both fields. The carver holder set
        // accepts a single holder, a tag or a list; feature steps are always a list.
        if !record
            .get("carvers")
            .is_some_and(|set| set.is_string() || set.is_array() || set.is_object())
        {
            return Err(DecodeError::Field("carvers"));
        }
        if !record.get("features").is_some_and(Value::is_array) {
            return Err(DecodeError::Field("features"));
        }
        Ok(Self(value))
    }

    pub fn into_json(self) -> Value {
        self.0
    }
}

/// Identity within an immutable biome registry snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BiomeId(pub u32);

#[derive(Debug)]
pub enum BiomeError {
    Decode(DecodeError),
    Index(IndexError),
    Sample(SampleError),
    OutsideVolume,
}
impl std::fmt::Display for BiomeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Decode(e) => e.fmt(f),
            Self::Index(e) => e.fmt(f),
            Self::Sample(e) => e.fmt(f),
            Self::OutsideVolume => f.write_str("biome query is outside the prepared quart volume"),
        }
    }
}
impl std::error::Error for BiomeError {}
impl From<DecodeError> for BiomeError {
    fn from(e: DecodeError) -> Self {
        Self::Decode(e)
    }
}
impl From<IndexError> for BiomeError {
    fn from(e: IndexError) -> Self {
        Self::Index(e)
    }
}
impl From<SampleError> for BiomeError {
    fn from(e: SampleError) -> Self {
        Self::Sample(e)
    }
}

#[derive(Debug)]
pub struct MultiNoiseBiomeSource {
    parameters: ClimateIndex<String>,
}
impl MultiNoiseBiomeSource {
    pub fn new(entries: Vec<(ParameterPoint, String)>) -> Result<Self, BiomeError> {
        let entries = entries
            .into_iter()
            .map(|(point, name)| Ok((point, identifier(&name)?)))
            .collect::<Result<_, DecodeError>>()?;
        Ok(Self {
            parameters: ClimateIndex::new(entries)?,
        })
    }
    /// Sunrise's quantized import format, not the vanilla biome definition
    /// codec: [{"coordinates":[[min,max]; six],"offset":long,"biome":id}].
    /// Built-in numeric tables stay private and are authenticated by the runner.
    pub fn from_quantized_json(json: &str) -> Result<Self, BiomeError> {
        let input: serde_json::Value = serde_json::from_str(json).map_err(DecodeError::from)?;
        let mut entries = Vec::new();
        for entry in input
            .as_array()
            .ok_or(DecodeError::Field("biome parameter list"))?
        {
            let raw = entry["coordinates"]
                .as_array()
                .filter(|v| v.len() == 6)
                .ok_or(DecodeError::Field("climate coordinates"))?;
            let mut coordinates = [Parameter::new(0, 0); 6];
            for (coordinate, raw) in coordinates.iter_mut().zip(raw) {
                let raw = raw
                    .as_array()
                    .filter(|v| v.len() == 2)
                    .ok_or(DecodeError::Field("climate interval"))?;
                *coordinate = Parameter::new(
                    raw[0]
                        .as_i64()
                        .ok_or(DecodeError::Field("climate minimum"))?,
                    raw[1]
                        .as_i64()
                        .ok_or(DecodeError::Field("climate maximum"))?,
                );
            }
            let offset = entry["offset"]
                .as_i64()
                .ok_or(DecodeError::Field("climate offset"))?;
            let biome = entry["biome"]
                .as_str()
                .ok_or(DecodeError::Field("biome identifier"))?;
            entries.push((
                ParameterPoint {
                    coordinates,
                    offset,
                },
                biome.to_owned(),
            ));
        }
        Self::new(entries)
    }
    pub fn parameters(&self) -> &ClimateIndex<String> {
        &self.parameters
    }
    pub fn resolver<'a>(&'a self, climate: &'a ClimateSampler) -> BiomeResolver<'a> {
        BiomeResolver {
            climate,
            search: self.parameters.searcher(),
        }
    }
}
#[derive(Debug)]
pub struct BiomeResolver<'a> {
    climate: &'a ClimateSampler,
    search: ClimateSearch<'a, String>,
}
impl<'a> BiomeResolver<'a> {
    pub fn sample(
        &mut self,
        context: &mut SamplerContext,
        quart: [i32; 3],
    ) -> Result<&'a str, BiomeError> {
        Ok(self
            .search
            .find(self.climate.sample(context, quart)?)?
            .as_str())
    }
    pub fn prepare_volume(
        &self,
        context: &mut SamplerContext,
        minimum: [i32; 3],
        extents: [i32; 3],
    ) -> Result<ClimateVolume, BiomeError> {
        Ok(self.climate.sample_volume(context, minimum, extents)?)
    }
    /// Query order shares the same previous-leaf state as scalar resolution.
    pub fn sample_prepared(
        &mut self,
        volume: &ClimateVolume,
        quart: [i32; 3],
    ) -> Result<&'a str, BiomeError> {
        let target = volume.target(quart).ok_or(BiomeError::OutsideVolume)?;
        Ok(self.search.find(target)?.as_str())
    }
}
