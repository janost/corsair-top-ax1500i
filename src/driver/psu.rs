use std::thread::sleep;
use std::time::Duration;
use rusb::{Context, UsbContext};
use crate::driver::device::ClaimedDevice;
use crate::driver::encode::{decode, encode};

#[derive(Clone, Copy)]
pub struct Config {
    pub vendor_id: u16,
    pub product_id: u16,
}

impl Config {
    pub fn default() -> Config {
        Config {
            vendor_id: 0x1b1c,
            product_id: 0x1c11,
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

#[derive(Clone, Debug)]
pub struct PsuReadings {
    pub bus: u8,
    pub address: u8,
    pub name: String,
    pub input_voltage: f64,
    pub input_current: f64,
    pub input_power: f64,
    pub rails: Vec<RailReadings>,
    pub twelve_v_pages: Vec<TwelveVPageReadings>,
    pub temp1: f64,
    pub temp2: f64,
    pub fan_speed: f64,
    pub uptime_hours: f64,
}

pub struct Psu {
    config: Config,
    claimed_device: ClaimedDevice,
    bus: u8,
    address: u8,
}

impl Psu {
    pub fn setup_all(context: &Context, config: Config) -> Vec<Psu> {
        let mut psus = Vec::new();
        for device in context.devices().unwrap().iter() {
            let device_desc = device.device_descriptor().unwrap();
            if device_desc.vendor_id() == config.vendor_id && device_desc.product_id() == config.product_id {
                let bus = device.bus_number();
                let address = device.address();
                let claimed_device = ClaimedDevice::claim(device, 0x00).expect("Claiming device failed");
                claimed_device.write_control();
                psus.push(Psu { config, claimed_device, bus, address });
            }
        }
        psus
    }

    pub fn get_bus(&self) -> u8 {
        self.bus
    }

    pub fn get_address(&self) -> u8 {
        self.address
    }

    fn write_encoded(&mut self, command: usize, msg: &[u8]) -> Vec<u8> {
        let encoded = encode(command, msg);
        let result1 = self.claimed_device.write_bulk(&encoded);
        if result1 != encoded.len() {
            // write failed silently in TUI mode
        }
        return self.read_and_decode();
    }

    fn read_and_decode(&mut self) -> Vec<u8> {
        let mut result = self.claimed_device.read_bulk();
        while result[result.len() - 1] != 0 {
            result.append(&mut self.claimed_device.read_bulk());
        }
        return decode(&result);
    }

    pub fn setup_dongle(&mut self) {
        Self::expect_zero(self.write_encoded(0, &[
            0x11, 0x02, 0x64, 0x00, 0x00, 0x00, 0x00,
        ]));
    }

    fn get_device_name(&mut self) -> String {
        let out = self.read_data_psu(0x07, 0x9a);
        String::from_utf8_lossy(&out).to_string()
    }

    fn read_data_psu(&mut self, len: u8, reg: u8) -> Vec<u8> {
        let header: [u8; 7] = [
            0x13, 0x03, 0x06, 0x01, 0x07, len, reg,
        ];
        Self::expect_zero(self.write_encoded(0, &header));
        Self::expect_ok(self.write_encoded(0, &[0x12]));
        return self.write_encoded(0, &[
            0x08, 0x07, len
        ]);
    }

    fn write_data_psu(&mut self, reg: u8, data: &[u8]) -> Vec<u8> {
        let header: [u8; 5] = [
            0x13, 0x01, 0x04, data.len() as u8, reg,
        ];
        let join: Vec<u8> = header.iter().chain(data.iter()).cloned().collect();
        Self::expect_zero(self.write_encoded(0, join.as_slice()));

        let msg4: [u8; 1] = [
            0x12
        ];
        return self.write_encoded(0, &msg4);
    }

    fn get_f64_register(&mut self, register: u8) -> f64 {
        let out = self.read_data_psu(0x02, register);
        return convert_byte_float(&out);
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

    fn get_input_voltage(&mut self) -> f64 {
        self.get_f64_register(0x88)
    }

    fn get_input_current(&mut self) -> f64 {
        self.get_f64_register(0x89)
    }

    fn get_input_power(&mut self) -> f64 {
        self.get_f64_register(0xee)
    }

    fn get_rail_voltage(&mut self) -> f64 {
        let out = self.read_data_psu(0x02, 0x8b);
        convert_byte_float(&out)
    }

    fn get_rail_current(&mut self) -> f64 {
        let out = self.read_data_psu(0x02, 0x8c);
        convert_byte_float(&out)
    }

    fn get_rail_watts(&mut self) -> f64 {
        let out = self.read_data_psu(0x02, 0x96);
        convert_byte_float(&out)
    }

    fn get_12v_rail_current(&mut self) -> f64 {
        let out = self.read_data_psu(0x02, 0xe8);
        convert_byte_float(&out)
    }

    fn get_12v_rail_power(&mut self) -> f64 {
        let out = self.read_data_psu(0x02, 0xe9);
        convert_byte_float(&out)
    }

    fn get_12v_rail_ocp_limit(&mut self) -> f64 {
        let out = self.read_data_psu(0x02, 0xea);
        convert_byte_float(&out)
    }

    fn get_fan_speed(&mut self) -> f64 {
        self.get_f64_register(0x90)
    }

    fn get_temp1(&mut self) -> f64 {
        self.get_f64_register(0x8e)
    }

    fn get_temp2(&mut self) -> f64 {
        self.get_f64_register(0x8d)
    }

    fn set_12v_page(&mut self, page_number: u8) -> u8 {
        let page: [u8; 1] = [page_number];
        self.write_data_psu(0xe7, &page);
        sleep(Duration::from_millis(2));
        let r = self.read_data_psu(0x01, 0xe7);
        if r.len() >= 1 { r[0] } else { 0 }
    }

    fn set_rail(&mut self, page_number: u8) -> u8 {
        let page: [u8; 1] = [page_number];
        self.write_data_psu(0x00, &page);
        sleep(Duration::from_millis(2));
        let r = self.read_data_psu(0x01, 0x00);
        if r.len() >= 1 { r[0] } else { 0 }
    }

    pub fn set_fan_mode(&mut self, mode: u8) {
        let mode: [u8; 1] = [mode];
        self.write_data_psu(0xf0, &mode);
    }

    pub fn set_fan_speed_percent(&mut self, speed: u8) {
        let percent: [u8; 1] = [speed];
        self.write_data_psu(0x3b, &percent);
    }

    pub fn read_all(&mut self) -> PsuReadings {
        let name = self.get_device_name();

        // Input values
        let input_voltage = self.get_input_voltage();
        let input_current = self.get_input_current();
        let input_power = self.get_input_power();

        // Rails (0=12V, 1=5V, 2=3.3V)
        let mut rails = Vec::new();
        for i in 0..3 {
            self.set_rail(i);
            self.set_12v_page(0);
            let voltage = self.get_rail_voltage();
            let current = self.get_rail_current();
            let power = self.get_rail_watts();
            rails.push(RailReadings { voltage, current, power });
        }

        // 12V pages
        let mut twelve_v_pages = Vec::new();
        for i in 0..12 {
            self.set_rail(0);
            self.set_12v_page(i);
            let voltage = self.get_rail_voltage();
            let current = self.get_12v_rail_current();
            let power = self.get_12v_rail_power();
            let ocp_limit = self.get_12v_rail_ocp_limit();
            if power > 0.0 {
                twelve_v_pages.push(TwelveVPageReadings {
                    page: i,
                    voltage,
                    current,
                    power,
                    ocp_limit,
                });
            }
        }

        self.set_rail(0);
        let uptime_hours = self.get_uptime_hours();
        let temp1 = self.get_temp1();
        let temp2 = self.get_temp2();
        let fan_speed = self.get_fan_speed();

        PsuReadings {
            bus: self.bus,
            address: self.address,
            name,
            input_voltage,
            input_current,
            input_power,
            rails,
            twelve_v_pages,
            temp1,
            temp2,
            fan_speed,
            uptime_hours,
        }
    }

    fn expect_zero(response: Vec<u8>) {
        if response.len() != 1 {
            // unexpected length
        } else if *response.get(0).unwrap() != 0 {
            // error reported
        }
    }

    fn expect_ok(response: Vec<u8>) {
        if response.len() != 2 {
            // unexpected length
        } else if *response.get(0).unwrap() != 0 {
            // error reported
        }
    }

    pub fn release(&mut self) {
        self.claimed_device.release();
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
    let base = 2.0 as f64;
    return (p2 as f64) * base.powf(p1 as f64);
}
