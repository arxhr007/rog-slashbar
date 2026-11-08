//! HID driver for the ASUS Slash bar (unofficial protocol, from G-Helper's SlashDevice.cs).

use crate::render::N;
use crate::state::log;
use hidapi::{HidApi, HidDevice};
use std::time::{Duration, Instant};

const VID: u16 = 0x0B05;
/// (product id, report id)
const CANDIDATES: [(u16, u8); 2] = [(0x193B, 0x5E), (0x19B6, 0x5D)];
const RESEND: Duration = Duration::from_secs(10);

/// 128-byte feature report: report id, command bytes, zero padding.
fn packet(report_id: u8, cmd: &[u8]) -> [u8; 128] {
    let mut p = [0u8; 128];
    p[0] = report_id;
    p[1..1 + cmd.len()].copy_from_slice(cmd);
    p
}

pub struct Bar {
    api: Option<HidApi>,
    dev: Option<HidDevice>,
    rid: u8,
    last: Option<[u8; N]>,
    last_sent: Instant,
}

impl Bar {
    pub fn new() -> Self {
        Bar { api: None, dev: None, rid: 0, last: None, last_sent: Instant::now() }
    }

    pub fn ok(&self) -> bool {
        self.dev.is_some()
    }

    fn send(&self, cmd: &[u8]) -> Result<(), String> {
        let dev = self.dev.as_ref().ok_or("no device")?;
        dev.send_feature_report(&packet(self.rid, cmd)).map_err(|e| e.to_string())
    }

    /// Find the HID interface that accepts Slash feature reports, then run the init sequence.
    pub fn connect(&mut self) -> bool {
        match self.api.as_mut() {
            Some(api) => {
                let _ = api.refresh_devices();
            }
            None => match HidApi::new() {
                Ok(api) => self.api = Some(api),
                Err(e) => {
                    log(&format!("hidapi init failed: {e}"));
                    return false;
                }
            },
        }
        let api = self.api.as_ref().expect("api just set");
        let mut found = None;
        'outer: for &(pid, rid) in &CANDIDATES {
            for info in api
                .device_list()
                .filter(|d| d.vendor_id() == VID && d.product_id() == pid)
            {
                let Ok(dev) = api.open_path(info.path()) else { continue };
                if dev.send_feature_report(&packet(rid, b"ASUS Tech.Inc.")).is_ok() {
                    found = Some((dev, rid, pid, info.usage_page()));
                    break 'outer;
                }
            }
        }
        let Some((dev, rid, pid, usage_page)) = found else { return false };
        self.dev = Some(dev);
        self.rid = rid;
        self.last = None;
        if let Err(e) = self.setup() {
            log(&format!("setup failed: {e}"));
            self.dev = None;
            return false;
        }
        log(&format!("connected {VID:04X}:{pid:04X} usage_page 0x{usage_page:04X}"));
        true
    }

    fn setup(&self) -> Result<(), String> {
        self.send(&[0xC2])?;
        self.send(&[0xD1, 0x01, 0x00, 0x01])?;
        self.send(&[0xD7, 0x00, 0x00, 0x01, 0xAC])?;
        self.send(&[0xD2, 0x02, 0x01, 0x08, 0xAB])?;
        self.send(&[0xD8, 0x02, 0x00, 0x01, 0x00])?; // enable
        self.send(&[0xD3, 0x03, 0x01, 0x08, 0xAB, 0xFF, 0x01, 0x01, 0x06, 0xFF, 0xFF, 0x00])?;
        self.send(&[0xD2, 0x02, 0x01, 0x08, 0xAC])?; // custom-pattern header
        self.send(&[0xD3, 0x03, 0x01, 0x08, 0xAC, 0xFF, 0xFF, 0x01, 0x05, 0xFF, 0xFF])?;
        self.send(&[0xD4, 0x00, 0x00, 0x01, 0xAC])?;
        Ok(())
    }

    /// Write a frame (physical segment order). Skips unchanged frames, with a periodic keep-alive.
    pub fn show(&mut self, frame: [u8; N]) {
        if self.last == Some(frame) && self.last_sent.elapsed() < RESEND {
            return;
        }
        let mut cmd = vec![0xD3, 0x00, 0x00, N as u8];
        cmd.extend_from_slice(&frame);
        match self.send(&cmd) {
            Ok(()) => {
                self.last = Some(frame);
                self.last_sent = Instant::now();
            }
            Err(e) => {
                log(&format!("write failed: {e}"));
                self.dev = None;
                self.last = None;
            }
        }
    }

    /// Blank the bar and disable it.
    pub fn off(&mut self) {
        if self.ok() {
            let mut cmd = vec![0xD3, 0x00, 0x00, N as u8];
            cmd.extend_from_slice(&[0u8; N]);
            let _ = self.send(&cmd);
            let _ = self.send(&[0xD8, 0x02, 0x00, 0x01, 0x80]);
        }
    }
}
