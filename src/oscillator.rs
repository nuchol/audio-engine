use std::f32::consts::TAU;

#[derive(Clone, Copy, Debug)]
pub enum Waveform {
    Sine,
    Square,
    Saw,
    Triangle,
}

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

pub struct Oscillator {
    freq: f32,
    amp: f32,
    waveform: Waveform,

    phase: f32,
    sample_rate: f32
}

impl Oscillator {
    pub fn new(waveform: Waveform, sample_rate: f32) -> Self {
        Self { freq: 440.0, amp: 0.5, waveform, phase: 0.0, sample_rate }
    }

    pub fn with_values(freq: f32, amp: f32, waveform: Waveform, sample_rate: f32) -> Self {
        Self { freq, amp, waveform, phase: 0.0, sample_rate }
    }

    pub fn next_sample(&mut self) -> f32 {
        self.phase = (self.phase + (self.freq / self.sample_rate)) % 1.0;
        self.waveform.sample(self.phase)
    }

    pub fn set_freq(&mut self, freq: f32) {
        self.freq = freq;
    }

    pub fn set_amp(&mut self, amp: f32) {
        self.amp = amp;
    }

    pub fn set_waveform(&mut self, waveform: Waveform) {
        self.waveform = waveform;
    }
}
