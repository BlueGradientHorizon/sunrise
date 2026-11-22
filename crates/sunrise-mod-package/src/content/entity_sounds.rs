//! Declarative sound selection; omission remains distinct from explicit silence.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EntitySounds {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient: Option<SoundOverride>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hurt: Option<SoundOverride>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub death: Option<SoundOverride>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<StepOverride>,
    /// `LivingEntity.getSoundVolume`: the ambient, hurt and death sounds' volume.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_volume"
    )]
    pub volume: Option<f32>,
    /// The base of `LivingEntity.getVoicePitch`, in place of the adult's 1.0.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_voice_pitch"
    )]
    pub voice_pitch: Option<f32>,
    /// `Mob.getAmbientSoundInterval`, in ticks. Any int: vanilla negates and
    /// compares whatever the method returns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient_interval: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum SoundOverride {
    Mute(Mute),
    Id(String),
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum Mute {
    #[serde(rename = "none")]
    None,
}
/// The step slot also takes `playStepSound`'s own `playSound(sound, volume, pitch)`.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(untagged)]
pub enum StepOverride {
    Mute(Mute),
    /// Played at the block step's volume 0.15 and pitch 1.0.
    Id(String),
    Sound(StepSound),
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StepSound {
    pub sound: String,
    #[serde(deserialize_with = "deserialize_step_volume")]
    pub volume: f32,
    #[serde(deserialize_with = "deserialize_step_pitch")]
    pub pitch: f32,
}
impl<'de> Deserialize<'de> for StepOverride {
    /// A string or the object form only: a derived struct would also read a
    /// positional array, and an untagged enum would hide which key is wrong.
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let value = serde_json::Value::deserialize(d)
            .map_err(|e| D::Error::custom(format_args!("sounds.step: {e}")))?;
        match value {
            serde_json::Value::String(id) if id == "none" => Ok(Self::Mute(Mute::None)),
            serde_json::Value::String(id) => Ok(Self::Id(id)),
            object @ serde_json::Value::Object(_) => StepSound::deserialize(object)
                .map(Self::Sound)
                .map_err(D::Error::custom),
            _ => Err(D::Error::custom(
                "sounds.step: a sound id, \"none\" or {\"sound\", \"volume\", \"pitch\"}",
            )),
        }
    }
}
/// A float as Java reads one into a `float`, refused unless finite, with its key.
fn finite<'de, D: serde::Deserializer<'de>>(d: D, key: &str) -> Result<f32, D::Error> {
    use serde::de::Error;
    let value =
        f64::deserialize(d).map_err(|e| D::Error::custom(format_args!("sounds.{key}: {e}")))?;
    let float = value as f32;
    if !float.is_finite() {
        return Err(D::Error::custom(format_args!(
            "sounds.{key}: {value:e} is not a finite float"
        )));
    }
    Ok(float)
}
fn deserialize_volume<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<f32>, D::Error> {
    finite(d, "volume").map(Some)
}
fn deserialize_voice_pitch<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<f32>, D::Error> {
    finite(d, "voice_pitch").map(Some)
}
fn deserialize_step_volume<'de, D: serde::Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    finite(d, "step.volume")
}
fn deserialize_step_pitch<'de, D: serde::Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    finite(d, "step.pitch")
}
impl EntitySounds {
    pub fn validate(&self, entity: &str) -> Result<(), String> {
        let step = match &self.step {
            Some(StepOverride::Id(id)) | Some(StepOverride::Sound(StepSound { sound: id, .. })) => {
                Some(id)
            }
            _ => None,
        };
        fn id(value: &Option<SoundOverride>) -> Option<&String> {
            match value {
                Some(SoundOverride::Id(id)) => Some(id),
                _ => None,
            }
        }
        for (slot, id) in [
            ("ambient", id(&self.ambient)),
            ("hurt", id(&self.hurt)),
            ("death", id(&self.death)),
            ("step", step),
        ] {
            let Some(id) = id else { continue };
            let valid = id.split_once(':').is_some_and(|(n, p)| {
                !n.is_empty()
                    && !p.is_empty()
                    && n.bytes().all(|b| {
                        b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b)
                    })
                    && p.bytes().all(|b| {
                        b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-/".contains(&b)
                    })
            });
            if !valid {
                return Err(format!(
                    "{entity}: sounds.{slot}: invalid full sound id {id:?}"
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "entity_sounds_tests.rs"]
mod tests;
