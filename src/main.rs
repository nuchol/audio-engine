use std::{f32::consts::TAU, sync::mpsc};
use std::thread;
use std::time::Duration;
use std::sync::mpsc::Sender;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

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
    fn sample(self, phase: f32) -> f32 {
        match self {
            Waveform::Sine => (phase * TAU).sin(),
            Waveform::Square => if phase < 0.5 { 1.0 } else { -1.0 },
            Waveform::Saw => 2.0 * phase - 1.0,
            Waveform::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
        }
    }
}

struct Voice {
    id: u32,
    freq: f32,
    amp: f32,
    wave: Waveform,
    phase: f32,
}

enum AudioCommand {
    NoteOn { id: u32, freq: f32, amp: f32, wave: Waveform },
    NoteOff { id: u32 },
}

struct AudioEngine {
    _stream: cpal::Stream,
    cmd_sender: Sender<AudioCommand>,
    next_id: u32,
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

        let sample_rate = config.sample_rate().0 as f32;
        let channels = config.channels() as usize;

        let (transmitter, reciever) = mpsc::channel::<AudioCommand>();

        let mut voices: Vec<Voice> = Vec::with_capacity(64);

        let stream = device
            .build_output_stream(
                &config.into(),
                move |data: &mut [f32], _| {
                    while let Ok(cmd) = reciever.try_recv() {
                        match cmd {
                            AudioCommand::NoteOn { id, freq, amp, wave } => {
                                voices.push(Voice { id, freq, amp, wave, phase: 0.0 })
                            },

                            AudioCommand::NoteOff { id } => voices.retain(|v| v.id != id),
                        }
                    }

                    for frame in data.chunks_mut(channels) {
                        let mut mix = 0.0;
                        for v in voices.iter_mut() {
                            mix += v.wave.sample(v.phase) * v.amp;
                            v.phase = (v.phase + v.freq / sample_rate) % 1.0;
                        }
                        frame.fill(mix.clamp(-1.0, 1.0));
                    }
                },
                |err| eprintln!("stream error: {err}"),
                None,
            )
            .expect("failed to build output stream");

        stream.play().expect("failed to start stream");
        Self { _stream: stream, cmd_sender: transmitter, next_id: 0 }
    }

    pub fn note_on(&mut self, freq: f32, amp: f32, wave: Waveform) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.cmd_sender.send(AudioCommand::NoteOn { id, freq, amp, wave }).unwrap();
        id
    }

    pub fn note_off(&self, id: u32) {
        self.cmd_sender.send(AudioCommand::NoteOff { id }).unwrap();
    }
}

fn main() {
    let mut engine = AudioEngine::new();

    for wave in WAVEFORMS {
        println!("Playing C major with {wave:?} wave");

        // C major chord
        let c = engine.note_on(261.63, 0.2, wave);
        let e = engine.note_on(329.63, 0.2, wave);
        let g = engine.note_on(392.00, 0.2, wave);

        thread::sleep(Duration::from_secs(2));

        engine.note_off(c);
        engine.note_off(e);
        engine.note_off(g);

        thread::sleep(Duration::from_secs(1));
    }
}
