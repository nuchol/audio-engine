use std::f32::consts::TAU;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

const FREQUENCY: f32 = 440.0;
const AMPLITUDE: f32 = 0.2;
const SECONDS_PER_WAVE: u64 = 2;

#[derive(Clone, Copy, Debug)]
enum Waveform {
    Sine,
    Square,
    Saw,
    Triangle,
}

const WAVEFORMS: [Waveform; 4] = [
    Waveform::Sine,
    Waveform::Square,
    Waveform::Saw,
    Waveform::Triangle,
];

impl Waveform {
    /// Sample the waveform at phase in [0, 1), output is [-1, 1].
    fn sample(self, phase: f32) -> f32 {
        match self {
            Waveform::Sine => (phase * TAU).sin(),
            Waveform::Square => if phase < 0.5 { 1.0 } else { -1.0 },
            Waveform::Saw => 2.0 * phase - 1.0,
            Waveform::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
        }
    }
}

fn main() {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .expect("no output device available");
    let config = device
        .default_output_config()
        .expect("no default output config");

    assert_eq!(
        config.sample_format(),
        cpal::SampleFormat::F32,
        "only f32 output is supported"
    );

    let sample_rate = config.sample_rate().0 as f32;
    let channels = config.channels() as usize;

    // Index into WAVEFORMS, shared between main thread and audio callback.
    let current = Arc::new(AtomicU8::new(0));
    let current_cb = Arc::clone(&current);

    let mut phase = 0.0f32;
    let stream = device
        .build_output_stream(
            &config.into(),
            move |data: &mut [f32], _| {
                let wave = WAVEFORMS[current_cb.load(Ordering::Relaxed) as usize];
                for frame in data.chunks_mut(channels) {
                    let value = wave.sample(phase) * AMPLITUDE;
                    frame.fill(value);
                    phase = (phase + FREQUENCY / sample_rate) % 1.0;
                }
            },
            |err| eprintln!("stream error: {err}"),
            None,
        )
        .expect("failed to build output stream");

    stream.play().expect("failed to start stream");

    for (i, wave) in WAVEFORMS.iter().enumerate() {
        println!("Playing {wave:?} at {FREQUENCY} Hz");
        current.store(i as u8, Ordering::Relaxed);
        thread::sleep(Duration::from_secs(SECONDS_PER_WAVE));
    }
}
