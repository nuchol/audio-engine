#[derive(Debug)]
enum Stage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

pub struct AdsrSettings {
    attack: f32,
    decay: f32,
    sustain: f32,
    release: f32,
}

impl AdsrSettings {
    pub fn new(attack: f32, decay: f32, sustain: f32, release: f32) -> Self {
        Self { attack, decay, sustain, release}
    }
}

const MIN_TIME: f32 = 0.001;

pub struct Adsr { stage: Stage, level: f32, release_level: f32 }
impl Adsr {
    pub fn new() -> Self { Self { stage: Stage::Idle, level: 0.0, release_level: 0.0 } }
    pub fn note_on(&mut self)  { self.stage = Stage::Attack; }

    pub fn note_off(&mut self) { 
        if !matches!(self.stage, Stage::Idle) {
            self.release_level = self.level;
            self.stage = Stage::Release;
        }
    }

    pub fn is_idle(&self) -> bool { matches!(self.stage, Stage::Idle) }
    pub fn is_releasing(&self) -> bool { matches!(self.stage, Stage::Release) }

    pub fn next(&mut self, settings: &AdsrSettings, sample_rate: f32) -> f32 {
        let attack = settings.attack.max(MIN_TIME) * sample_rate;
        let decay = settings.decay.max(MIN_TIME) * sample_rate;
        let release = settings.release.max(MIN_TIME) * sample_rate;
        let sustain = settings.sustain.clamp(0.0, 1.0);

        match self.stage {
            Stage::Idle => self.level = 0.0,
            Stage::Attack => {
                self.level += 1.0 / attack;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = Stage::Decay;
                }
            },
            Stage::Decay => {
                self.level -= (1.0 - sustain) / decay;
                if self.level <= sustain {
                    self.level = sustain;
                    self.stage = Stage::Sustain;
                }
            }
            Stage::Sustain => self.level = sustain,
            Stage::Release => {
                self.level -= self.release_level / release;
                if self.level <= 0.0 {
                    self.level = 0.0;
                    self.stage = Stage::Idle;
                }
            }
        };

        self.level
    }
}
