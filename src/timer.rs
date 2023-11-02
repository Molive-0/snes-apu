use std::num::NonZeroU8;

#[derive(Debug, Clone, Copy)]
pub struct Timer {
    resolution: usize,
    is_running: bool,
    ticks: usize,
    target: Option<NonZeroU8>,
    counter_low: u8,
    counter_high: u8,
}

impl Timer {
    pub fn new(resolution: usize) -> Timer {
        Timer {
            resolution: resolution,
            is_running: false,
            ticks: 0,
            target: None,
            counter_low: 0,
            counter_high: 0,
        }
    }

    pub fn tick(&mut self) {
        if !self.is_running {
            return;
        }
        self.ticks += 1;
        if self.ticks > self.resolution {
            self.ticks -= self.resolution;

            self.counter_low += 1;
            if let Some(target) = self.target {
                if self.counter_low == target.into() {
                    self.counter_high += 1;
                    self.counter_low = 0;
                }
            }
        }
    }

    pub fn set_start_stop_bit(&mut self, value: bool) {
        if value && !self.is_running {
            self.ticks = 0;
            self.counter_low = 0;
        }
        self.is_running = value;
    }

    pub fn set_target(&mut self, value: u8) {
        self.target = NonZeroU8::new(value)
    }

    pub fn read_counter(&mut self) -> u8 {
        let ret = self.counter_high & 0x0f;
        self.counter_high = 0;
        ret
    }
}
