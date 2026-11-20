pub enum NoteEvent {
    NoteOn { note: u8 },
    NoteOff { note: u8 },
}

pub trait Instrument: Send {
    fn handle_event(&mut self, event: NoteEvent);
    fn set_param(&mut self, id: u32, value: f32);
    fn process(&mut self, out: &mut [f32]);
}

pub trait Processor {
    fn process(&mut self, out: &mut [f32]);
}

