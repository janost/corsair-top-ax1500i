use crate::driver::PsuReadings;

pub struct App {
    pub readings: Vec<PsuReadings>,
    pub power_history: Vec<Vec<f64>>,
    pub temp_history: Vec<Vec<f64>>,
    pub fan_history: Vec<Vec<f64>>,
    pub total_power_history: Vec<f64>,
    pub should_quit: bool,
    pub tick_count: u64,
    pub tick_rate_ms: u64,
    pub is_ax1600i: bool,
}

const HISTORY_LEN: usize = 60;

impl App {
    pub fn new(num_psus: usize) -> App {
        App {
            readings: Vec::new(),
            power_history: vec![Vec::with_capacity(HISTORY_LEN); num_psus],
            temp_history: vec![Vec::with_capacity(HISTORY_LEN); num_psus],
            fan_history: vec![Vec::with_capacity(HISTORY_LEN); num_psus],
            total_power_history: Vec::with_capacity(HISTORY_LEN),
            should_quit: false,
            tick_count: 0,
            tick_rate_ms: 1000,
            is_ax1600i: false,
        }
    }

    pub fn increase_tick_rate(&mut self) {
        if self.tick_rate_ms > 250 {
            self.tick_rate_ms -= 250;
        }
    }

    pub fn decrease_tick_rate(&mut self) {
        if self.tick_rate_ms < 5000 {
            self.tick_rate_ms += 250;
        }
    }

    pub fn update(&mut self, readings: Vec<PsuReadings>) {
        self.tick_count += 1;

        for (i, reading) in readings.iter().enumerate() {
            if i < self.power_history.len() {
                self.power_history[i].push(reading.input_power);
                if self.power_history[i].len() > HISTORY_LEN {
                    self.power_history[i].remove(0);
                }

                self.temp_history[i].push(reading.temp1);
                if self.temp_history[i].len() > HISTORY_LEN {
                    self.temp_history[i].remove(0);
                }

                self.fan_history[i].push(reading.fan_speed);
                if self.fan_history[i].len() > HISTORY_LEN {
                    self.fan_history[i].remove(0);
                }
            }
        }

        let total: f64 = readings.iter().map(|r| r.input_power).sum();
        self.total_power_history.push(total);
        if self.total_power_history.len() > HISTORY_LEN {
            self.total_power_history.remove(0);
        }

        self.readings = readings;
    }

    pub fn total_power(&self) -> f64 {
        self.readings.iter().map(|r| r.input_power).sum()
    }

    pub fn total_12v_power(&self) -> f64 {
        self.readings
            .iter()
            .map(|r| {
                if !r.rails.is_empty() {
                    r.rails[0].power
                } else {
                    0.0
                }
            })
            .sum()
    }
}
