use std::f32::consts::TAU;
use std::thread;
use std::time::Duration;

use cpal::{Device, Host, SupportedStreamConfig};
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

struct AudioEngine {
    host: Host,
    device: Device,
    config: SupportedStreamConfig,
}


impl AudioEngine {
    pub fn new() -> Self {
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

        Self { host, device, config }
    }

    pub fn play(&self, freq: f32, amp: f32, duration: u64, wave: Waveform) {
        let sample_rate = self.config.sample_rate().0 as f32;
        let channels = self.config.channels() as usize;

        let mut phase = 0.0f32;
        let stream = self.device
            .build_output_stream(
                &self.config.clone().into(),
                move |data: &mut [f32], _| {
                    for frame in data.chunks_mut(channels) {
                        let value = wave.sample(phase) * amp;
                        frame.fill(value);
                        phase = (phase + freq / sample_rate) % 1.0;
                    }
                },
                |err| eprintln!("stream error: {err}"),
                None,
            )
            .expect("failed to build output stream");

        stream.play().expect("failed to start stream");
        thread::sleep(Duration::from_secs(duration));
    }
}

fn main() {
    let engine = AudioEngine::new();
    for (i, wave) in WAVEFORMS.iter().enumerate() {
        println!("Playing {wave:?} at {FREQUENCY} Hz");
        engine.play(FREQUENCY, AMPLITUDE, SECONDS_PER_WAVE, *wave);
    }
}
