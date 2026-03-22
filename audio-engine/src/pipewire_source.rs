//! `PipeWire` audio capture source — pipewire-rs 0.8.0.
//!
//! Uses `RefCell` for the analyser so the single-threaded `PipeWire`
//! process callback can borrow it mutably without unsafe code.

use crate::{AudioError, AudioSource, AudioSpectrum, analysis::AudioAnalyser};
use parking_lot::RwLock;
use pipewire::properties::properties;
use std::{
    cell::RefCell,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tracing::{error, info};

/// `PipeWire` system-audio capture source.
pub struct PipeWireSource {
    spectrum: Arc<RwLock<AudioSpectrum>>,
    stop: Arc<AtomicBool>,
    _thread: std::thread::JoinHandle<()>,
}

impl PipeWireSource {
    /// # Errors
    /// Returns [`AudioError::PipeWireInit`] if `PipeWire` cannot initialise.
    pub fn new(fft_size: usize, _target: Option<&str>) -> Result<Self, AudioError> {
        pipewire::init();

        let spectrum = Arc::new(RwLock::new(AudioSpectrum::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let spectrum_thread = Arc::clone(&spectrum);
        let stop_thread = Arc::clone(&stop);

        let thread = std::thread::Builder::new()
            .name("pipewire-audio".into())
            .spawn(move || {
                if let Err(e) = run_pipewire_loop(&spectrum_thread, &stop_thread, fft_size) {
                    error!(error = %e, "PipeWire audio thread error");
                }
            })
            .map_err(|e| AudioError::PipeWireInit(e.to_string()))?;

        info!(fft_size, "PipeWire audio source started");
        Ok(Self {
            spectrum,
            stop,
            _thread: thread,
        })
    }
}

impl AudioSource for PipeWireSource {
    fn current_spectrum(&self) -> Result<AudioSpectrum, AudioError> {
        Ok(*self.spectrum.read())
    }
    fn source_name(&self) -> &'static str {
        "pipewire"
    }
}

impl Drop for PipeWireSource {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

fn run_pipewire_loop(
    spectrum: &Arc<RwLock<AudioSpectrum>>,
    stop: &Arc<AtomicBool>,
    fft_size: usize,
) -> Result<(), AudioError> {
    use pipewire::{
        context::Context,
        main_loop::MainLoop,
        spa::{param::audio::AudioFormat, pod::Pod, utils::Direction},
        stream::{Stream, StreamFlags},
    };

    // 0.8.0: MainLoop::new takes Option<&DictRef>; None = default properties.
    let main_loop = MainLoop::new(None).map_err(|e| AudioError::PipeWireInit(e.to_string()))?;
    let context = Context::new(&main_loop).map_err(|e| AudioError::PipeWireInit(e.to_string()))?;
    let core = context
        .connect(None)
        .map_err(|e| AudioError::PipeWireInit(e.to_string()))?;

    let analyser = RefCell::new(
        AudioAnalyser::new(fft_size).map_err(|e| AudioError::FftConfig(e.to_string()))?,
    );

    // Build stream properties.  In pipewire-rs 0.8.0 the `properties!` macro
    // is exported at the crate root as a `#[macro_export]` item, but Rust's
    // path-based macro invocation (`crate::macro_name!`) requires the macro to
    // be in scope.  Import it explicitly to avoid the path resolution error.
    let props = properties! {
        *pipewire::keys::MEDIA_TYPE     => "Audio",
        *pipewire::keys::MEDIA_CATEGORY => "Capture",
        *pipewire::keys::MEDIA_ROLE     => "Music",
    };

    // 0.8.0: Stream has no generic parameter.
    let stream = Stream::new(&core, "momoi-audio", props)
        .map_err(|e| AudioError::PipeWireInit(e.to_string()))?;

    // SPA format pod: stereo F32LE at 48 kHz.
    let values: Vec<u8> = pipewire::spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &pipewire::spa::pod::Value::Object(pipewire::spa::pod::object!(
            pipewire::spa::utils::SpaTypes::ObjectParamFormat,
            pipewire::spa::param::ParamType::EnumFormat,
            pipewire::spa::pod::property!(
                pipewire::spa::param::format::FormatProperties::MediaType,
                Id,
                pipewire::spa::param::format::MediaType::Audio
            ),
            pipewire::spa::pod::property!(
                pipewire::spa::param::format::FormatProperties::MediaSubtype,
                Id,
                pipewire::spa::param::format::MediaSubtype::Raw
            ),
            pipewire::spa::pod::property!(
                pipewire::spa::param::format::FormatProperties::AudioFormat,
                Id,
                AudioFormat::F32LE
            ),
            pipewire::spa::pod::property!(
                pipewire::spa::param::format::FormatProperties::AudioRate,
                Int,
                48_000
            ),
            pipewire::spa::pod::property!(
                pipewire::spa::param::format::FormatProperties::AudioChannels,
                Int,
                2
            ),
        )),
    )
    .expect("SPA pod serialisation")
    .0
    .into_inner();

    let param = Pod::from_bytes(&values).expect("valid POD");

    let spectrum_cb = Arc::clone(spectrum);
    let _listener = stream
        .add_local_listener_with_user_data(())
        .process(move |stream, (): &mut ()| {
            let Some(mut buf) = stream.dequeue_buffer() else {
                return;
            };
            let datas = buf.datas_mut();
            let Some(data) = datas.first_mut() else {
                return;
            };
            let Some(bytes) = data.data() else { return };

            let samples: Vec<f32> = bytes
                .chunks_exact(8)
                .map(|c| {
                    let l = f32::from_le_bytes([c[0], c[1], c[2], c[3]]);
                    let r = f32::from_le_bytes([c[4], c[5], c[6], c[7]]);
                    (l + r) * 0.5
                })
                .collect();

            *spectrum_cb.write() = analyser.borrow_mut().analyse(&samples);
        })
        .register()
        .map_err(|e| AudioError::PipeWireInit(e.to_string()))?;

    stream
        .connect(
            Direction::Input,
            None,
            StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS | StreamFlags::RT_PROCESS,
            &mut [param],
        )
        .map_err(|e| AudioError::PipeWireInit(e.to_string()))?;

    // 0.8.0: MainLoop has no .iterate() method.
    // Drive the loop by calling iterate() on the underlying LoopRef.
    let loop_ = main_loop.loop_();
    while !stop.load(Ordering::Acquire) {
        loop_.iterate(std::time::Duration::from_millis(10));
    }

    info!("PipeWire audio thread stopped");
    Ok(())
}
