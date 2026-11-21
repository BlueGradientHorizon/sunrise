//! Host method declarations; access defaults and game forwarders are generated.
use crate::*;
/// Audio streams for [`crate::ClientHost`]. Streams use a pool separate from vanilla's
/// sound instances, fire no sound hooks and require a mod call on a live client from setup
/// onward. The mixer resamples to the output's rate and applies the stream's volume, category
/// and master gain, `muffle_sounds`, and the position and linear range fade for spatial mono.
/// Muted and unplaced streams still advance. Keep the returned handle for as long as playback
/// should continue; dropping it stops the stream.
#[sunrise_mod_api_macros::host_area]
pub trait AudioClientHost {
    /// Side: client.
    /// Gives the mixer a native source of interleaved signed 16-bit PCM, like NeoForge's
    /// `SoundInstance.getStream`. The mixer takes ownership and calls [`AudioStream::read`]
    /// serially on the output's shared worker as it needs frames, with no `Host` or world borrow.
    /// The device callback requests PCM but never calls or waits for the source. Returning `0`
    /// signals EOF so already-queued frames drain before playback stops; return `n > 0` without
    /// writing to the zeroed buffer for temporary silence. An async or network producer bridges
    /// through its own nonblocking buffer inside [`AudioStream::read`].
    ///
    /// Refuses with [`AudioRefusal`] when playback is unavailable, the spec is invalid, a stream
    /// limit is reached, or the pull worker cannot take the source.
    ///
    /// ```
    /// use mod_api::{AudioStream, SoundSource, StreamSpec};
    /// # use mod_api::{AudioRefusal, AudioStreamRef, ClientHost};
    ///
    /// struct Clip(std::vec::IntoIter<i16>);
    /// impl AudioStream for Clip {
    ///     fn read(&mut self, pcm: &mut [i16]) -> usize {
    ///         let mut frames = 0;
    ///         for (out, sample) in pcm.iter_mut().zip(self.0.by_ref()) {
    ///             *out = sample;
    ///             frames += 1; // One sample per frame in this mono stream.
    ///         }
    ///         frames // 0 signals EOF.
    ///     }
    /// }
    ///
    /// # fn open(host: &dyn ClientHost, samples: Vec<i16>) -> Result<AudioStreamRef, AudioRefusal> {
    /// let spec = StreamSpec::new(SoundSource::Music, 1, 48_000);
    /// host.open_audio_stream(&spec, Box::new(Clip(samples.into_iter())))
    /// # }
    /// ```
    #[access(ClientSetup)]
    fn open_audio_stream(
        &self,
        _spec: &StreamSpec,
        _source: Box<dyn AudioStream>,
    ) -> Result<AudioStreamRef, AudioRefusal>;
}
