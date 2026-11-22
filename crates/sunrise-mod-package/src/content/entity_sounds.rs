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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<f32>,
    /// The base of `LivingEntity.getVoicePitch`, in place of the adult's 1.0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice_pitch: Option<f32>,
    /// `Mob.getAmbientSoundInterval`, in ticks.
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
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
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
    pub volume: f32,
    pub pitch: f32,
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
        let (step_volume, step_pitch) = match &self.step {
            Some(StepOverride::Sound(step)) => (Some(step.volume), Some(step.pitch)),
            _ => (None, None),
        };
        for (key, value) in [
            ("step.volume", step_volume),
            ("step.pitch", step_pitch),
            ("volume", self.volume),
            ("voice_pitch", self.voice_pitch),
        ] {
            if let Some(value) = value.filter(|value| !value.is_finite()) {
                return Err(format!("{entity}: sounds.{key}: {value} is not finite"));
            }
        }
        // At least one tick between ambient sounds.
        if let Some(interval) = self.ambient_interval.filter(|&interval| interval < 1) {
            return Err(format!(
                "{entity}: sounds.ambient_interval: {interval} is below 1"
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "entity_sounds_tests.rs"]
mod tests;
