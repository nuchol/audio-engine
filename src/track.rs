use crate::processors::{ Processor, Instrument, NoteEvent };

pub struct Track {
    pub instrument: Box<dyn Instrument>,
    pub gain: f32,
    pub pan: f32,
    pub buffer: Vec<f32>,
    pub effects: Vec<Box<dyn Processor + Send>>,
}

impl Track {
    pub fn new(instrument: Box<dyn Instrument>, buf_size: usize) -> Self {
        Self {
            instrument,
            gain: 0.0,
            pan: 0.0,
            buffer: vec![0.0; buf_size],
            effects: Vec::new(),
        }
    }

    pub fn handle_event(&mut self, event: NoteEvent) {
        self.instrument.handle_event(event);
    }
}
