//! Host method declarations; access defaults and game forwarders are generated.
use crate::*;
/// Audio streams and microphone capture for [`crate::ClientHost`]. Both playback openers use a
/// pool separate from vanilla's sound instances, fire no sound hooks and require a mod call on a
/// live client from setup onward. The mixer resamples to the output's rate and applies the stream's
/// volume, category and master gain, `muffle_sounds`, and the position and linear range fade for
/// spatial mono. Muted and unplaced streams still advance. Keep the returned handle for as long as
/// playback should continue; dropping it stops the stream.
#[sunrise_mod_api_macros::host_area]
pub trait AudioClientHost {
    /// Side: client.
    /// Gives the mixer a native source of interleaved signed 16-bit PCM, like NeoForge's
    /// `SoundInstance.getStream`. The mixer takes ownership and calls [`AudioStream::read`]
    /// serially on the output's shared worker as it needs frames, with no `Host` or world borrow.
    /// The device callback requests PCM but never calls or waits for the source. A short or dry
    /// read is retried on later demand. EOF lets ready frames drain before playback stops.
    /// [`AudioStreamRef::push`] always returns zero on this handle.
    ///
    /// Refuses with [`AudioRefusal`] when playback is unavailable, the spec is invalid, a stream
    /// limit is reached, or the pull worker cannot take the source.
    ///
    /// ```
    /// use mod_api::{AudioRead, AudioStream, SoundSource, StreamSpec};
    /// # use mod_api::{AudioRefusal, AudioStreamRef, ClientHost};
    ///
    /// struct Clip(std::vec::IntoIter<i16>);
    /// impl AudioStream for Clip {
    ///     fn read(&mut self, pcm: &mut [i16]) -> AudioRead {
    ///         let mut frames = 0;
    ///         for (out, sample) in pcm.iter_mut().zip(self.0.by_ref()) {
    ///             *out = sample;
    ///             frames += 1; // One sample per frame in this mono stream.
    ///         }
    ///         if frames == 0 { AudioRead::Eof } else { AudioRead::Frames(frames) }
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

    /// Side: client.
    /// Opens a stream for an async producer, such as a network payload handler. Use
    /// [`AudioStreamRef::push`] from one producer thread per stream. It returns the whole
    /// frames the bounded ring accepted, without waiting for the mixer or a tick. Playback
    /// starts with the first available frame and plays silence when dry. Stop or drop the
    /// handle to end the stream.
    ///
    /// Refuses with [`AudioRefusal`] when playback is unavailable, the spec is invalid or a
    /// stream limit is reached.
    ///
    /// ```
    /// use mod_api::{AudioRefusal, AudioStreamRef, ClientHost, SoundSource, StreamSpec};
    ///
    /// fn open(host: &dyn ClientHost) -> Result<AudioStreamRef, AudioRefusal> {
    ///     host.open_push_audio_stream(&StreamSpec::new(SoundSource::Voice, 2, 48_000))
    /// }
    ///
    /// // On the producer thread: drop the suffix or retry it later.
    /// fn push<'a>(stream: &AudioStreamRef, stereo_pcm: &'a [i16]) -> &'a [i16] {
    ///     let frames = stream.push(stereo_pcm) as usize;
    ///     &stereo_pcm[frames * 2..]
    /// }
    /// ```
    #[access(ClientSetup)]
    fn open_push_audio_stream(&self, _spec: &StreamSpec) -> Result<AudioStreamRef, AudioRefusal>;

    /// Side: client.
    /// Lists the client's available audio input device names. Returns an empty list when no input
    /// devices exist or client audio is disabled.
    ///
    /// Refuses with [`MicRefusal::OnServer`] on a server, [`MicRefusal::OsDenied`] if the
    /// operating system denies microphone permission, [`MicRefusal::Unavailable`] outside client
    /// setup or later, or [`MicRefusal::DeviceFailed`] if device enumeration fails.
    ///
    /// ```
    /// use mod_api::{ClientHost, MicRefusal};
    ///
    /// fn list(host: &dyn ClientHost) -> Result<Vec<String>, MicRefusal> {
    ///     host.microphone_devices()
    /// }
    /// ```
    #[access(ClientSetup)]
    fn microphone_devices(&self) -> Result<Vec<String>, MicRefusal>;

    /// Side: client.
    /// Opens live PCM capture from the system default or named input device directly on the
    /// client. Returns a client-local [`MicRef`] readable from any thread off the OS audio
    /// callback: [`MicRef::read`] fills a caller-provided slice with whole interleaved signed
    /// 16-bit PCM frames from a bounded [`MIC_RING_MAX_MS`] ms ring that drops the oldest frames
    /// on overflow. Capture stops when [`MicRef::close`] is called or the handle is dropped. The
    /// engine never records or transmits captured audio.
    ///
    /// Refuses with [`MicRefusal`] when called on a server, outside client setup or later, with
    /// an invalid [`MicSpec`], when the OS denies permission, or when no matching device can be
    /// opened.
    ///
    /// ```
    /// use mod_api::{ClientHost, MicRef, MicRefusal, MicSpec};
    ///
    /// fn open(host: &dyn ClientHost) -> Result<MicRef, MicRefusal> {
    ///     host.open_microphone(&MicSpec::new(1, 48_000))
    /// }
    ///
    /// // On any thread: read whole interleaved frames without blocking on a tick or callback.
    /// fn drain(mic: &MicRef, pcm: &mut [i16]) -> usize {
    ///     mic.read(pcm) as usize
    /// }
    /// ```
    #[access(ClientSetup)]
    fn open_microphone(&self, _spec: &MicSpec) -> Result<MicRef, MicRefusal>;
}
