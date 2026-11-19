use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

mod oscillator;
mod adsr;
use crate::adsr::{ Adsr, AdsrSettings };
use crate::oscillator::{Oscillator, Waveform};

const MAX_TRACKS: usize = 16;

pub enum NoteEvent {
    NoteOn { note: u8 },
    NoteOff { note: u8 },
}

pub trait Instrument: Send {
    fn handle_event(&mut self, event: NoteEvent);
    fn set_param(&mut self, id: u32, value: f32);
    // fn process(&mut self, out: &mut [f32]);
    fn process(&mut self) -> f32;
}

struct Params {
    // filter
    cutoff: f32,            // base cutoff in Hz, e.g. 2000.0
    resonance: f32,         // 0.0..1.0
    filter_env_amount: f32, // how far the filter env pushes cutoff (in octaves)
    lfo_to_cutoff: f32,     // how far the LFO pushes cutoff

    // envelopes (settings only)
    amp_env: AdsrSettings,
    flt_env: AdsrSettings,

    // oscillator
    waveform: Waveform,
    detune: f32,

    master_gain: f32,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            // filter
            cutoff: 2000.0,
            resonance: 0.2,
            filter_env_amount: 2.0,
            lfo_to_cutoff: 0.0,

            // envelopes (settings only)
            amp_env: AdsrSettings::new(0.01, 0.2, 0.8, 0.3),
            flt_env: AdsrSettings::new(0.01, 0.2, 0.8, 0.3),

            // oscillator
            waveform: Waveform::Saw,
            detune: 0.0,

            master_gain: 0.25,
        }
    }
}

struct Synth {
    voices: Vec<Voice>,
    params: Params,
    sample_rate: f32,

    voice_age: u32,
}

impl Synth {
    pub fn new(num_voices: u8, params: Params, sample_rate: f32) -> Self {
        Self {
            voices: (0..num_voices)
                .map(|_| Voice::new(params.waveform, sample_rate))
                .collect(),
            params,
            sample_rate,

            voice_age: 0,
        }
    }

    pub fn next_sample(&mut self) -> f32 {
        let mut mix = 0.0;
        for v in self.voices.iter_mut() {
            if !v.is_idle() {
                mix += v.next_sample(0.0, &self.params, self.sample_rate);
            }
        }
        // mix * self.params.master_gain
        mix
    }

    fn note_on(&mut self, note: u8) {
        self.voice_age += 1;
        let idx = self.find_voice(note);
        let v = &mut self.voices[idx];
        v.note = note;
        v.age = self.voice_age;
        v.start(note);
    }

    fn note_off(&mut self, note: u8) {
        for v in &mut self.voices {
            if v.note == note && !v.is_idle() && !v.is_releasing() {
                v.release();
            }
        }
    }

    fn find_voice(&self, note: u8) -> usize {
        let vs = &self.voices;
        if let Some(i) = vs.iter().position(|v| v.note == note && !v.is_idle()) { return i; }
        if let Some(i) = vs.iter().position(|v| v.is_idle()) { return i; }
        if let Some((i, _)) = vs.iter().enumerate()
            .filter(|(_, v)| v.is_releasing())
            .min_by_key(|(_, v)| v.age) { return i; }
        vs.iter().enumerate()
            .min_by_key(|(_, v)| v.age)
            .map(|(i, _)| i)
            .expect("synth has no voices")   // only fails if num_voices == 0
    }
}

impl Instrument for Synth {
    fn handle_event(&mut self, event: NoteEvent) {
        match event {
            NoteEvent::NoteOn { note } => self.note_on(note),
            NoteEvent::NoteOff { note } => self.note_off(note),
        }
    }

    // CORRECT
    // fn process(&mut self, out: &mut [f32]) {
    //     for s in out.iter_mut() { *s = self.next_sample(); }
    // }

    fn process(&mut self) -> f32 {
        self.next_sample()
    }

    fn set_param(&mut self, id: u32, value: f32) {
        
    }
}

struct Voice {
    osc: Oscillator,
    freq: f32,
    amp_env: Adsr,
    filter_env: Adsr,
    note: u8,

    age: u32,
}

impl Voice {
    pub fn new(waveform: Waveform, sample_rate: f32) -> Self {
        Self {
            osc: Oscillator::new(waveform, sample_rate),
            freq: 0.0,
            amp_env: Adsr::new(),
            filter_env: Adsr::new(),
            note: 0,
            age: 0,
        }
    }

    pub fn next_sample(&mut self, lfo: f32, p: &Params, sample_rate: f32) -> f32 {
        let mut sample = self.osc.next_sample();
        let amp = self.amp_env.next(&p.amp_env, sample_rate);
        sample *= amp;
        sample
    }

    fn start(&mut self, note: u8) {
        self.freq = 440.0 * 2_f32.powf((note as f32 - 69.0) / 12.0);
        self.osc.set_freq(self.freq);

        self.amp_env.note_on();
        self.filter_env.note_on();
    }

    fn release(&mut self) {
        self.amp_env.note_off();
        self.filter_env.note_off();
    }

    fn is_idle(&self) -> bool { self.amp_env.is_idle() }
    fn is_releasing(&self) -> bool { self.amp_env.is_releasing() }
}

struct Track {
    instrument: Box<dyn Instrument>,
    gain: f32,
    pan: f32,
    buffer: Vec<f32>,
}

impl Track {
    pub fn new(instrument: Box<dyn Instrument>) -> Self {
        Self {
            instrument,
            gain: 0.0,
            pan: 0.0,
            buffer: Vec::new(),
        }
    }

    pub fn handle_event(&mut self, event: NoteEvent) {
        self.instrument.handle_event(event);
    }
}

pub enum EngineCommand {
    Note { track: u32, event: NoteEvent },
    Param { track: u32, id: u32, value: f32 },
    TrackGain { track: u32, gain: f32 },
    AddTrack(Box<Track>),
    RemoveTrack(u32),
}

struct Engine {
    tracks: Vec<Track>,
    garbage_tx: Sender<Track>,
}

impl Engine {
    pub fn new(garbage_tx: Sender<Track>) -> Self {
        Self {
            tracks: Vec::with_capacity(MAX_TRACKS),
            garbage_tx,
        }
    }

    pub fn process(&mut self) -> f32 {
        let mut out = 0.0;
        for track in self.tracks.iter_mut() {
            out += track.instrument.process();
        }
        out
    }

    pub fn handle_event(&mut self, cmd: EngineCommand) {
        match cmd {
            EngineCommand::Note { track, event } => {
                self.tracks[track as usize].handle_event(event);
            },
            EngineCommand::TrackGain { track, gain } => {},
            EngineCommand::Param { track, id, value } => {},
            EngineCommand::AddTrack(t) => self.tracks.push(*t),
            EngineCommand::RemoveTrack(id) => {
                let t = self.tracks.swap_remove(id as usize);
                let _ = self.garbage_tx.send(t);    // freed on the UI thread
            }
        }
    }
}

struct AudioEngine {
    _stream: cpal::Stream,
    cmd_tx: Sender<EngineCommand>,
    garbage_rx: Receiver<Track>,

    sample_rate: f32,
}

impl AudioEngine {
    pub fn new() -> Self {
        let host = cpal::default_host();
        let device = host.default_output_device().expect("no output device");
        let config = device.default_output_config().expect("no default config");
        let sample_rate = config.sample_rate().0 as f32;
        let channels = config.channels() as usize;

        let (cmd_tx, cmd_rx) = mpsc::channel::<EngineCommand>();
        let (garbage_tx, garbage_rx) = mpsc::channel::<Track>();

        let mut engine = Engine::new(garbage_tx);

        let stream = device
            .build_output_stream(
                &config.into(),
                move |data: &mut [f32], _| {
                    while let Ok(cmd) = cmd_rx.try_recv() {
                        engine.handle_event(cmd);
                    }

                    for frame in data.chunks_mut(channels) {
                        frame.fill(engine.process().clamp(-1.0, 1.0));
                    }
                },
                |err| eprintln!("stream error: {err}"),
                None,
            )
            .expect("failed to build output stream");

        stream.play().expect("failed to start stream");
        Self { _stream: stream, cmd_tx, garbage_rx, sample_rate }
    }

    pub fn send(&self, cmd: EngineCommand) { let _ = self.cmd_tx.send(cmd); }

    pub fn collect_garbage(&self) {
        while let Ok(track) = self.garbage_rx.try_recv() { drop(track); }
    }

    pub fn get_sample_rate(&self) -> f32 { self.sample_rate }
}

fn main() {
    let engine = AudioEngine::new();
    let mut instrument = Synth::new(8, Params::default(), engine.sample_rate);
    let mut track = Track::new(Box::new(instrument));
    engine.send(EngineCommand::AddTrack(Box::new(track)));

    engine.send(EngineCommand::Note{track: 0, event: NoteEvent::NoteOn { note: 60 }});
    engine.send(EngineCommand::Note{track: 0, event: NoteEvent::NoteOn { note: 64 }});
    // engine.send(EngineCommand::Note{track: 0, event: NoteEvent::NoteOn { note: 67 }});

    thread::sleep(Duration::from_secs(4));

    engine.send(EngineCommand::Note{track: 0, event: NoteEvent::NoteOff { note: 60 }});
    engine.send(EngineCommand::Note{track: 0, event: NoteEvent::NoteOff { note: 64 }});
    // engine.send(EngineCommand::Note{track: 0, event: NoteEvent::NoteOff { note: 67 }});

    thread::sleep(Duration::from_secs(3));
}
