use std::sync::mpsc::{ self, Receiver, Sender };
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

mod oscillator;
mod adsr;
mod track;
mod synth;
mod processors;

use crate::track::Track;
use crate::synth::{ Synth, Params };
use crate::processors::NoteEvent;

const BLOCK_SIZE: usize = 16;
const MAX_TRACKS: usize = 16;

pub enum EngineCommand {
    Note { track: u32, event: NoteEvent },
    Param { track: u32, id: u32, value: f32 },
    TrackGain { track: u32, gain: f32 },
    AddTrack(Box<Track>),
    RemoveTrack(u32),
}

// Lives on the autio thread
struct AudioTread {
    tracks: Vec<Track>,
    garbage_tx: Sender<Track>,
    block_size: usize,

    l_data: Vec<f32>,
    r_data: Vec<f32>,
}

impl AudioTread {
    pub fn new(garbage_tx: Sender<Track>, block_size: usize) -> Self {
        Self {
            tracks: Vec::with_capacity(MAX_TRACKS),
            garbage_tx,
            block_size,

            l_data: vec![0.0f32; block_size],
            r_data: vec![0.0f32; block_size],
        }
    }
    
    pub fn process_loop(
        &mut self,
        data: &mut [f32],
        channels: usize,
        cmd_rx: &Receiver<EngineCommand>,
    ) {
        while let Ok(cmd) = cmd_rx.try_recv() {
            self.handle_event(cmd);
        }

        for chunk in data.chunks_mut(self.block_size * channels) {
            let frames = chunk.len() / channels;
            self.process_tracks();

            let (l, r) = (&mut self.l_data[..frames], &mut self.r_data[..frames]);

            for (i, frame) in chunk.chunks_mut(channels).enumerate() {
                match channels {
                    1 => frame[0] = 0.5 * (l[i] + r[i]),
                    _ => {
                        frame[0] = l[i].clamp(-1.0, 1.0);
                        frame[1] = r[i].clamp(-1.0, 1.0);
                        for extra in &mut frame[2..] { *extra = 0.0; }
                    },
                }
            }
        }
    }

    fn process_tracks(&mut self) {
        self.l_data.fill(0.0);
        self.r_data.fill(0.0);

        let n = self.l_data.len();

        for track in self.tracks.iter_mut() {
            let buf = &mut track.buffer[..n];
            track.instrument.process(buf);
            // for fx in &mut t.effects { fx.process(buf); }

            // Linear pan law
            let (gain_l, gain_r) = (track.pan, 1. - track.pan);

            for i in 0..n {
                self.l_data[i] += buf[i] * gain_l;
                self.r_data[i] += buf[i] * gain_r;
            }
        }
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
                let _ = self.garbage_tx.send(t);
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
    pub fn new(block_size: usize) -> Self {
        let host = cpal::default_host();
        let device = host.default_output_device().expect("no output device");
        let config = device.default_output_config().expect("no default config");
        let sample_rate = config.sample_rate().0 as f32;
        let channels = config.channels() as usize;

        let (cmd_tx, cmd_rx) = mpsc::channel::<EngineCommand>();
        let (garbage_tx, garbage_rx) = mpsc::channel::<Track>();

        let mut process_thread = AudioTread::new(garbage_tx, block_size);

        let stream = device
            .build_output_stream(
                &config.into(),
                move |data: &mut [f32], _| {
                    process_thread.process_loop(data, channels, &cmd_rx);
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
    let engine = AudioEngine::new(BLOCK_SIZE);
    let instrument = Synth::new(8, Params::default(), engine.sample_rate);
    let track = Track::new(Box::new(instrument), BLOCK_SIZE);

    engine.send(EngineCommand::AddTrack(Box::new(track)));

    engine.send(EngineCommand::Note{track: 0, event: NoteEvent::NoteOn { note: 60 }});
    engine.send(EngineCommand::Note{track: 0, event: NoteEvent::NoteOn { note: 64 }});
    engine.send(EngineCommand::Note{track: 0, event: NoteEvent::NoteOn { note: 67 }});

    thread::sleep(Duration::from_millis(500));

    engine.send(EngineCommand::Note{track: 0, event: NoteEvent::NoteOff { note: 60 }});
    engine.send(EngineCommand::Note{track: 0, event: NoteEvent::NoteOff { note: 64 }});
    engine.send(EngineCommand::Note{track: 0, event: NoteEvent::NoteOff { note: 67 }});

    thread::sleep(Duration::from_millis(500));
}
