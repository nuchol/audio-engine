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

struct Note {
    id: u32,
    freq: f32,
    amp: f32,
    wave: Waveform,
    on: bool,
    on_time: f32,
    off_time: f32,
    phase: f32,
}

impl Note {
    pub fn new(id: u32, freq: f32, amp: f32, wave: Waveform) -> Self {
        Self {
            id,
            freq,
            amp,
            wave,
            on: true,
            on_time: 0.0,
            off_time: 0.0,
            phase: 0.0,
        }
    }
}

#[derive(Clone, Copy)]
struct Envelope {
    attack_ms: f32,
    decay_ms: f32,
    release_ms: f32,

    sustain_fq: f32,
}

impl Envelope {
    pub fn new(attack_ms: f32, decay_ms: f32, release_ms: f32, sustain_fq: f32) -> Self {
        Self { attack_ms, decay_ms, release_ms, sustain_fq }
    }

    pub fn sample(&self, note: Note) -> f32 {
        // Attack
        let mut value = (note.on_time as f32 / self.attack_ms).clamp(0.0, 1.0);

        // Decay
        let decay_timer = (note.on_time - self.attack_ms).max(0.) + note.off_time;
        value *= 1. - decay_timer / self.decay_ms;

        // Release
        if !note.on { value *= 1. - note.off_time / self.release_ms }
        value
    }
}

enum AudioCommand {
    NoteOn { note: Note },
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

        let mut notes: Vec<Note> = Vec::with_capacity(64);

        let stream = device
            .build_output_stream(
                &config.into(),
                move |data: &mut [f32], _| {
                    while let Ok(cmd) = reciever.try_recv() {
                        match cmd {
                            AudioCommand::NoteOn { note } => { notes.push(note) },
                            AudioCommand::NoteOff { id } => notes.retain(|n| n.id != id),
                        }
                    }

                    for frame in data.chunks_mut(channels) {
                        let mut mix = 0.0;
                        for n in notes.iter_mut() {
                            mix += n.wave.sample(n.phase) * n.amp;
                            n.phase = (n.phase + (n.freq / sample_rate)) % 1.0;

                            let delta = n.freq / sample_rate;
                            if n.on { n.on_time +=  delta } else { n.on_time += delta }
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

    pub fn note_on(&mut self, freq: f32, amp: f32, wave: Waveform, envelope: Envelope) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        let note = Note::new(id, freq, amp, wave);
        self.cmd_sender.send(AudioCommand::NoteOn{ note }).unwrap();
        id
    }

    pub fn note_off(&self, id: u32) {
        self.cmd_sender.send(AudioCommand::NoteOff { id }).unwrap();
    }
}

fn main() {
    let mut engine = AudioEngine::new();

    let env = Envelope::new(1000.0, 1000.0, 1000.0, 200.0);

    let c = engine.note_on(261.63, 0.2, Waveform::Sine, env);
    let e = engine.note_on(329.63, 0.2, Waveform::Sine, env);
    let g = engine.note_on(392.00, 0.2, Waveform::Sine, env);

    loop {}

    // for wave in WAVEFORMS {
    //     println!("Playing C major with {wave:?} wave");
    //
    //     let env = Envelope::new(1000.0, 1000.0, 1000.0, 200.0);
    //
    //     // C major chord
    //     let c = engine.note_on(261.63, 0.2, wave, env);
    //     let e = engine.note_on(329.63, 0.2, wave, env);
    //     let g = engine.note_on(392.00, 0.2, wave, env);
    //
    //     thread::sleep(Duration::from_secs(2));
    //
    //     engine.note_off(c);
    //     engine.note_off(e);
    //     engine.note_off(g);
    //
    //     thread::sleep(Duration::from_secs(1));
    // }
}
