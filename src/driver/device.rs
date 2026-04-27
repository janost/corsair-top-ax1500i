use std::io::{Read, Write};
use std::sync::Mutex;
use std::thread::sleep;
use std::time::Duration;

use serialport::{ClearBuffer, SerialPort, TTYPort};

pub struct ClaimedDevice {
    port: Mutex<TTYPort>,
    path: String,
}

impl ClaimedDevice {
    pub fn claim(path: &str) -> Result<ClaimedDevice, serialport::Error> {
        let port = serialport::new(path, 115_200)
            .data_bits(serialport::DataBits::Eight)
            .stop_bits(serialport::StopBits::One)
            .parity(serialport::Parity::None)
            .flow_control(serialport::FlowControl::None)
            .timeout(Duration::from_millis(50))
            .open_native()?;
        sleep(Duration::from_millis(100));
        Ok(ClaimedDevice {
            port: Mutex::new(port),
            path: path.to_string(),
        })
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    /// Drain any leftover input bytes — call before issuing a new request to
    /// ensure responses don't get desynced from previous transactions.
    pub fn drain_input(&self) {
        let port = self.port.lock().unwrap();
        let _ = port.clear(ClearBuffer::Input);
    }

    /// Read exactly `expected` bytes from the wire. Returns whatever bytes
    /// were read (may be short on timeout). No trailer eating — leftover
    /// bytes are flushed by `drain_input()` at the start of the next call.
    pub fn read_exact_or_timeout(&self, expected: usize) -> Vec<u8> {
        let mut port = self.port.lock().unwrap();
        let mut out = Vec::with_capacity(expected);
        let mut buf = [0u8; 256];
        while out.len() < expected {
            let want = (expected - out.len()).min(buf.len());
            match port.read(&mut buf[..want]) {
                Ok(0) => break,
                Ok(n) => out.extend_from_slice(&buf[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => break,
                Err(_) => break,
            }
        }
        out
    }

    pub fn write_bulk(&self, data: &[u8]) -> usize {
        let mut port = self.port.lock().unwrap();
        port.write(data).unwrap_or(0)
    }

    pub fn release(&mut self) {
        // Port is released when dropped.
    }
}
