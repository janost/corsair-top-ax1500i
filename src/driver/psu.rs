use std::thread::sleep;
use std::time::Duration;

use crate::driver::device::ClaimedDevice;
use crate::driver::encode::{decode, encode};

#[derive(Clone)]
pub struct Config {
    pub device_paths: Vec<String>,
}

impl Config {
    pub fn default() -> Config {
        Config {
            device_paths: vec!["/dev/ttyUSB0".to_string()],
        }
    }
}

#[derive(Clone, Debug)]
pub struct RailReadings {
    pub voltage: f64,
    pub current: f64,
    pub power: f64,
}

#[derive(Clone, Debug)]
pub struct TwelveVPageReadings {
    pub page: u8,
    pub voltage: f64,
    pub current: f64,
    pub power: f64,
    pub ocp_limit: f64,
}

#[derive(Clone, Debug, Copy, PartialEq)]
pub enum FanMode {
    Auto,
    Fixed,
    Unknown,
}

#[derive(Clone, Debug)]
pub struct PsuReadings {
    pub bus: u8,
    pub address: u8,
    pub name: String,
    pub input_voltage: f64,
    pub input_current: f64,
    pub input_power: f64,
    pub output_power: f64,
    pub efficiency: f64,
    pub cable_type_20a: bool,
    pub fan_mode: FanMode,
    pub rails: Vec<RailReadings>,
    pub twelve_v_pages: Vec<TwelveVPageReadings>,
    pub temp1: f64,
    pub temp2: f64,
    pub fan_speed: f64,
    pub uptime_hours: f64,
}

pub struct Psu {
    claimed_device: ClaimedDevice,
    path: String,
    last_pages: Vec<TwelveVPageReadings>,
}

impl Psu {
    pub fn setup_all(config: &Config) -> Vec<Psu> {
        let mut psus = Vec::new();
        for path in &config.device_paths {
            match ClaimedDevice::claim(path) {
                Ok(claimed) => {
                    psus.push(Psu {
                        claimed_device: claimed,
                        path: path.clone(),
                        last_pages: Vec::new(),
                    });
                }
                Err(e) => {
                    eprintln!("Failed to open {}: {}", path, e);
                }
            }
        }
        psus
    }

    pub fn get_path(&self) -> &str {
        &self.path
    }

    /// Send `msg` (decoded) and read back the response containing exactly
    /// `expected_data_bytes` decoded bytes. Wire response size = 2*N + 2
    /// (1 header + 2*N nibble bytes + 1 trailer).
    fn transact(&mut self, command: usize, msg: &[u8], expected_data_bytes: usize) -> Vec<u8> {
        let encoded = encode(command, msg);
        let _ = self.claimed_device.write_bulk(&encoded);
        let wire_len = expected_data_bytes * 2 + 1;
        let raw = self.claimed_device.read_exact_or_timeout(wire_len);
        if raw.is_empty() {
            return Vec::new();
        }
        let decoded = decode(&raw);
        let mut out = decoded;
        out.truncate(expected_data_bytes);
        out
    }

    pub fn setup_dongle(&mut self) {
        // Read dongle name (variable length, up to ~64 bytes)
        let _ = self.transact(0, &[0x02], 64);
        // Configure SMBus bridge: 100 kHz clock; expect 1-byte ack
        let _ = self.transact(0, &[0x11, 0x02, 0x64, 0x00, 0x00, 0x00, 0x00], 1);
        // Read dongle version (3 bytes)
        let _ = self.transact(0, &[0x00], 3);
        // Read MFR_MODEL (matches cpsumon — primes the controller)
        let _ = self.read_data_psu(0x07, 0x9a);
    }

    fn get_device_name(&mut self) -> String {
        let out = self.read_data_psu(0x07, 0x9a);
        String::from_utf8_lossy(&out).trim_end_matches('\0').to_string()
    }

    /// SMBus block read: write [0x13, 0x03, 0x06, 0x01, 0x07, len, reg] (1-byte ack),
    /// then write [0x08, 0x07, len] and read `len + 1` data bytes (cpsumon convention —
    /// the extra byte ensures the bridge's full response is consumed; we use only `len`).
    fn read_data_psu(&mut self, len: u8, reg: u8) -> Vec<u8> {
        let header: [u8; 7] = [0x13, 0x03, 0x06, 0x01, 0x07, len, reg];
        let _ = self.transact(0, &header, 1);
        let resp = self.transact(0, &[0x08, 0x07, len], (len as usize) + 1);
        // Use only the first `len` bytes (the extra was for framing).
        resp.into_iter().take(len as usize).collect()
    }

    /// SMBus byte/block write: [0x13, 0x01, 0x04, len+1, reg, data...]
    /// Returns the 1-byte ack.
    fn write_data_psu(&mut self, reg: u8, data: &[u8]) -> Vec<u8> {
        let mut frame: Vec<u8> = Vec::with_capacity(5 + data.len());
        frame.push(0x13);
        frame.push(0x01);
        frame.push(0x04);
        frame.push((data.len() + 1) as u8);
        frame.push(reg);
        frame.extend_from_slice(data);
        self.transact(0, &frame, 1)
    }

    fn get_f64_register(&mut self, register: u8) -> f64 {
        let out = self.read_data_psu(0x02, register);
        convert_byte_float(&out)
    }

    fn get_uptime_hours(&mut self) -> f64 {
        let out = self.read_data_psu(0x02, 0xd2);
        if out.len() >= 2 {
            let seconds = (out[0] as i64) + ((out[1] as i64) << 8);
            seconds as f64 / (60.0 * 60.0)
        } else {
            0.0
        }
    }

    fn get_input_voltage(&mut self) -> f64 { self.get_f64_register(0x88) }
    fn get_input_current(&mut self) -> f64 { self.get_f64_register(0x89) }
    /// Register 0x97 is the actual input-power reading on AX1500i (PMBus
    /// READ_PIN helper). cpsumon averages it with V*I for noise resistance.
    fn get_input_power_raw(&mut self) -> f64 { self.get_f64_register(0x97) }

    fn get_rail_voltage(&mut self) -> f64 { self.get_f64_register(0x8b) }
    fn get_rail_current(&mut self) -> f64 { self.get_f64_register(0x8c) }
    fn get_rail_watts(&mut self)   -> f64 { self.get_f64_register(0x96) }

    fn get_12v_rail_current(&mut self)   -> f64 { self.get_f64_register(0xe8) }
    fn get_12v_rail_power(&mut self)     -> f64 { self.get_f64_register(0xe9) }
    fn get_12v_rail_ocp_limit(&mut self) -> f64 { self.get_f64_register(0xea) }

    fn get_fan_speed(&mut self) -> f64 { self.get_f64_register(0x90) }
    fn get_temp1(&mut self) -> f64 { self.get_f64_register(0x8e) }
    fn get_temp2(&mut self) -> f64 { self.get_f64_register(0x8d) }

    /// Write to PMBus PAGE register (0x00), then verify with readback.
    fn set_main_page(&mut self, page: u8) -> bool {
        let _ = self.write_data_psu(0x00, &[page]);
        sleep(Duration::from_millis(2));
        let r = self.read_data_psu(0x01, 0x00);
        r.first().copied() == Some(page)
    }

    /// Write to vendor 12V virtual page register (0xe7), then verify with readback.
    fn set_12v_page(&mut self, page: u8) -> bool {
        let _ = self.write_data_psu(0xe7, &[page]);
        sleep(Duration::from_millis(2));
        let r = self.read_data_psu(0x01, 0xe7);
        r.first().copied() == Some(page)
    }

    pub fn release(&mut self) {
        self.claimed_device.release();
    }

    pub fn read_all(&mut self) -> PsuReadings {
        let name = self.get_device_name();
        let is_ax1500 = name.contains("AX1500");

        // Match cpsumon's read order: fan_mode (0xf0), fan_speed (0x90),
        // temp (0x8e), then set_main_page(0) + power readings.
        let fan_mode_resp = self.read_data_psu(0x01, 0xf0);
        let fan_mode = match fan_mode_resp.first() {
            Some(0) => FanMode::Auto,
            Some(_) => FanMode::Fixed,
            None => FanMode::Unknown,
        };
        let fan_speed = self.get_fan_speed();
        let temp1 = self.get_temp1();

        // cpsumon's read order: 0x97 → unk1, 0x89 → current, 0x88 → voltage,
        // 0xee → outputpower (raw), then 0xf2 → cabletype (AX1500i).
        // input_power = (unk1 + V*I) / 2.
        // output_power overwritten via calibration formula based on V & input_power.
        self.set_main_page(0);
        let pin_raw = self.get_input_power_raw();
        let input_current = self.get_input_current();
        let input_voltage = self.get_input_voltage();
        let _output_raw = self.get_f64_register(0xee);  // read but discard (overwritten)
        let cable_type_20a = if is_ax1500 {
            let resp = self.read_data_psu(0x01, 0xf2);
            resp.first().copied().unwrap_or(0) != 0
        } else {
            false
        };
        let input_power = (pin_raw + input_voltage * input_current) / 2.0;
        let mut output_power = ax1500_calibrated_output(input_voltage, input_power);
        if output_power > input_power * 0.99 {
            output_power = input_power * 0.99;
        }
        let efficiency = if input_power > 0.0 {
            (output_power / input_power) * 100.0
        } else {
            0.0
        };
        let temp2 = self.get_temp2();

        // Rails (0=12V, 1=5V, 2=3.3V) — main PMBus page
        let mut rails = Vec::new();
        for i in 0..3u8 {
            self.set_main_page(i);
            self.set_12v_page(0);
            let voltage = self.get_rail_voltage();
            let current = self.get_rail_current();
            let power = self.get_rail_watts();
            rails.push(RailReadings { voltage, current, power });
        }

        // 12V virtual pages (0..11). Stay on main page 0.
        // If a page write fails (controller refuses to decrement on AX1500i across cycles),
        // we keep the previous reading for that page rather than emitting garbage.
        self.set_main_page(0);
        let mut twelve_v_pages: Vec<TwelveVPageReadings> = Vec::new();
        for i in 0..12u8 {
            let page_ok = self.set_12v_page(i);
            if page_ok {
                let voltage = self.get_rail_voltage();
                let current = self.get_12v_rail_current();
                let power = self.get_12v_rail_power();
                let ocp_limit = self.get_12v_rail_ocp_limit();
                twelve_v_pages.push(TwelveVPageReadings {
                    page: i,
                    voltage,
                    current,
                    power,
                    ocp_limit,
                });
            } else if let Some(stale) = self.last_pages.iter().find(|p| p.page == i) {
                twelve_v_pages.push(stale.clone());
            }
        }
        self.last_pages = twelve_v_pages.clone();

        self.set_main_page(0);
        let uptime_hours = self.get_uptime_hours();

        PsuReadings {
            bus: 0,
            address: 0,
            name,
            input_voltage,
            input_current,
            input_power,
            output_power,
            efficiency,
            cable_type_20a,
            fan_mode,
            rails,
            twelve_v_pages,
            temp1,
            temp2,
            fan_speed,
            uptime_hours,
        }
    }
}

/// AX1500i output-power calibration formula from cpsumon. Returns
/// `input_power` unchanged if the input is outside the calibrated range.
fn ax1500_calibrated_output(voltage: f64, input_power: f64) -> f64 {
    if voltage < 170.0 && input_power < 259.0 {
        0.9151 * input_power - 8.5209
    } else if voltage >= 170.0 && input_power < 254.0 {
        0.9394 * input_power - 62.289
    } else {
        input_power
    }
}

fn convert_byte_float(data: &[u8]) -> f64 {
    if data.len() < 2 {
        return 0.0;
    }
    let mut p1 = ((data[1] as i32) >> 3) & 31;
    if p1 > 15 {
        p1 -= 32;
    }
    let mut p2 = ((data[1] as i32) & 7) * 256 + (data[0] as i32);
    if p2 > 1024 {
        p2 = -(65536 - (p2 | 63488));
    }
    let base = 2.0_f64;
    (p2 as f64) * base.powf(p1 as f64)
}
