/// Deterministic clock: time advances by exactly `1/fps` per `advance()`.
#[derive(Debug, Clone)]
pub struct FixedClock {
    fps: f64,
    frame: u64,
}

impl FixedClock {
    pub fn new(fps: f64) -> Self {
        Self { fps, frame: 0 }
    }

    /// Current time in seconds.
    pub fn now(&self) -> f64 {
        self.frame as f64 / self.fps
    }

    pub fn advance(&mut self) {
        self.frame += 1;
    }
}
