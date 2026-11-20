use crate::adsr::{ Adsr, AdsrSettings };
use crate::oscillator::{ Oscillator, Waveform };
use crate::processors::{ Instrument, NoteEvent };

pub struct Params {
    // filter
    cutoff: f32,
    resonance: f32,
    filter_env_amount: f32,
    lfo_to_cutoff: f32,

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

pub struct Synth {
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

        mix * self.params.master_gain
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

    fn process(&mut self, out: &mut [f32]) {
        for s in out.iter_mut() { *s = self.next_sample(); }
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
