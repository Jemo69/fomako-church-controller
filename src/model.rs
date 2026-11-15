//! App model: cameras, pages, image settings, persistence.

use rune::TextEdit;
use serde::{Deserialize, Serialize};

use crate::visca::Protocol;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Control,
    Image,
    Picture,
    Cameras,
    Help,
}

impl Page {
    pub fn title(self) -> &'static str {
        match self {
            Page::Control => "Control",
            Page::Image => "Image",
            Page::Picture => "Picture Edit",
            Page::Cameras => "Cameras",
            Page::Help => "Help",
        }
    }
    pub fn all() -> [Page; 5] {
        [Page::Control, Page::Image, Page::Picture, Page::Cameras, Page::Help]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CameraConfig {
    pub name: String,
    pub ip: String,
    pub protocol: ProtoSer,
    pub address: u8,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProtoSer {
    SonyUdp,
    Tcp,
}

impl ProtoSer {
    pub fn to_protocol(self) -> Protocol {
        match self {
            ProtoSer::SonyUdp => Protocol::SonyUdp,
            ProtoSer::Tcp => Protocol::IpViscaTcp,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            ProtoSer::SonyUdp => "Sony VISCA UDP :52381",
            ProtoSer::Tcp => "IP-VISCA TCP :5678",
        }
    }
    pub fn port(self) -> u16 {
        match self {
            ProtoSer::SonyUdp => 52381,
            ProtoSer::Tcp => 5678,
        }
    }
}

impl Default for CameraConfig {
    fn default() -> Self {
        CameraConfig {
            name: "Pulpit".into(),
            ip: "192.168.5.163".into(),
            protocol: ProtoSer::SonyUdp,
            address: 1,
        }
    }
}

pub struct App {
    pub cameras: Vec<CameraConfig>,
    pub selected: usize,
    pub page: Page,
    pub dark: bool,

    pub pan_speed: u8,
    pub tilt_speed: u8,
    pub zoom_speed: u8,

    pub status: String,
    pub log: Vec<String>,

    // camera form edits
    pub name_edit: TextEdit,
    pub ip_edit: TextEdit,

    // image settings (UI state, applied to camera on demand)
    pub ae_mode: u8, // 0 auto,1 manual,2 shutter,3 iris,4 bright
    pub shutter: u8,
    pub iris: u8,
    pub gain_limit: u8,
    pub bright_lvl: u8,
    pub expcomp_on: bool,
    pub expcomp_lvl: u8,
    pub backlight: bool,
    pub wdr: u8,
    pub nr2d: u8,
    pub nr3d: u8,
    pub flicker: u8, // 0 off 1 50Hz 2 60Hz
    pub gamma: u8,
    pub lowlight: bool,
    pub aperture: u8,   // sharpness 0..15
    pub brightness: u8,  // 0..14 (maps to 0x00..0x0E scaled)
    pub contrast: u8,   // 0..14
    pub saturation: u8,  // 0..14 (60%..200%)
    pub wb_mode: u8,     // 0 auto .. 11
    pub awb_sens: u8,    // 0 low 1 normal 2 high
    pub r_gain: u8,
    pub b_gain: u8,
    pub flip: u8, // 0 off 1 H 2 V 3 HV
    pub bw: bool,

    // picture-edit preview modifiers (0..100)
    pub pic_bright: i32,
    pub pic_contrast: i32,
    pub pic_sat: i32,
    pub pic_sharp: i32,
    pub preset_sel: u8,
}

impl App {
    pub fn new() -> Self {
        let cameras = load_cameras();
        let mut s = App {
            cameras,
            selected: 0,
            page: Page::Control,
            dark: true,
            pan_speed: 14,
            tilt_speed: 12,
            zoom_speed: 4,
            status: "Ready. Pick a camera, then drive it.".into(),
            log: vec![],
            name_edit: TextEdit::new("New Camera"),
            ip_edit: TextEdit::new("192.168.5.163"),
            ae_mode: 0,
            shutter: 10,
            iris: 8,
            gain_limit: 8,
            bright_lvl: 8,
            expcomp_on: false,
            expcomp_lvl: 7,
            backlight: false,
            wdr: 0,
            nr2d: 3,
            nr3d: 5,
            flicker: 0,
            gamma: 2,
            lowlight: false,
            aperture: 8,
            brightness: 7,
            contrast: 7,
            saturation: 7,
            wb_mode: 0,
            awb_sens: 1,
            r_gain: 32,
            b_gain: 32,
            flip: 0,
            bw: false,
            pic_bright: 0,
            pic_contrast: 0,
            pic_sat: 0,
            pic_sharp: 0,
            preset_sel: 1,
        };
        if s.cameras.is_empty() {
            s.cameras.push(CameraConfig::default());
        }
        s.push_log("FoMaKo controller started (VISCA 52381 / 5678, admin/admin)".into());
        s
    }

    pub fn current(&self) -> &CameraConfig {
        &self.cameras[self.selected.min(self.cameras.len().saturating_sub(1))]
    }

    pub fn push_log(&mut self, msg: String) {
        self.log.push(msg);
        if self.log.len() > 40 {
            let excess = self.log.len() - 40;
            self.log.drain(0..excess);
        }
    }

    pub fn set_status(&mut self, msg: String) {
        self.status = msg.clone();
        self.push_log(msg);
    }

    pub fn save(&self) {
        save_cameras(&self.cameras);
    }

    pub fn ae_byte(&self) -> u8 {
        match self.ae_mode {
            1 => 0x03,
            2 => 0x0A,
            3 => 0x0B,
            4 => 0x0D,
            _ => 0x00,
        }
    }
    pub fn ae_label(&self) -> &'static str {
        match self.ae_mode {
            1 => "Manual",
            2 => "Shutter Pri",
            3 => "Iris Pri",
            4 => "Bright",
            _ => "Full Auto",
        }
    }
    pub fn wb_label(&self) -> &'static str {
        match self.wb_mode {
            0 => "Auto",
            1 => "3000K",
            2 => "4000K",
            3 => "OnePush",
            4 => "OnePushTrig",
            5 => "Manual",
            6 => "6500K",
            7 => "3500K",
            8 => "4500K",
            9 => "5500K",
            10 => "6000K",
            11 => "7000K",
            _ => "?",
        }
    }
    pub fn wb_byte(&self) -> u8 {
        // Manual table mapping (same order as inquiry list):
        // 00 Auto, 01 3000K? Actually manual CAM_WB Mode pq=00..0B:
        // From inquiry: 00 Auto, 01 3000K? Let's use direct index.
        self.wb_mode.min(11)
    }
}

fn config_path() -> std::path::PathBuf {
    // Keep next to binary / cwd so church PC is portable (USB stick friendly).
    std::path::PathBuf::from("fomako-cameras.json")
}

fn load_cameras() -> Vec<CameraConfig> {
    // 1) cwd file, 2) exe dir file, else defaults (2 cams: pulpit + wide)
    for p in [config_path(), exe_dir_file()] {
        if let Ok(data) = std::fs::read_to_string(&p) {
            if let Ok(v) = serde_json::from_str::<Vec<CameraConfig>>(&data) {
                if !v.is_empty() {
                    return v;
                }
            }
        }
    }
    vec![
        CameraConfig {
            name: "Pulpit".into(),
            ip: "192.168.5.163".into(),
            protocol: ProtoSer::SonyUdp,
            address: 1,
        },
        CameraConfig {
            name: "Wide / Choir".into(),
            ip: "192.168.5.164".into(),
            protocol: ProtoSer::SonyUdp,
            address: 2,
        },
    ]
}

fn exe_dir_file() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("fomako-cameras.json")))
        .unwrap_or_else(config_path)
}

fn save_cameras(cams: &[CameraConfig]) {
    if let Ok(j) = serde_json::to_string_pretty(cams) {
        let _ = std::fs::write(config_path(), j);
        // also try exe dir (may fail without perms; ignore)
        let _ = std::fs::write(exe_dir_file(), serde_json::to_string_pretty(cams).unwrap_or_default());
    }
}
