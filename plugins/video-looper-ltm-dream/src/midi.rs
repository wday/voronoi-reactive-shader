// ── midi.rs ── MIDI CC output for Ableton delay sync ──
//
// Sends subdivision and feedback as MIDI CC messages through a virtual
// MIDI port named "Dream LTM". If the port isn't found, MIDI is silently
// disabled — everything else still works.

use midir::{MidiOutput, MidiOutputConnection};

const PORT_NAME: &str = "Dream LTM";
const MIDI_CHANNEL: u8 = 0; // Channel 1

// CC assignments
pub const CC_SUBDIVISION: u8 = 20;
pub const CC_FEEDBACK: u8 = 21;

pub struct MidiOut {
    conn: Option<MidiOutputConnection>,
    last_subdivision: u8,
    last_feedback: u8,
}

impl MidiOut {
    pub fn new() -> Self {
        let conn = Self::try_connect();
        Self {
            conn,
            last_subdivision: 255, // force first send
            last_feedback: 255,
        }
    }

    fn try_connect() -> Option<MidiOutputConnection> {
        let output = MidiOutput::new("Dream LTM Output").ok()?;
        let ports = output.ports();

        for port in &ports {
            if let Ok(name) = output.port_name(port) {
                tracing::info!(port_name = %name, "found MIDI port");
                if name.contains(PORT_NAME) {
                    match output.connect(port, "dream-ltm") {
                        Ok(conn) => {
                            tracing::info!("connected to MIDI port: {}", name);
                            return Some(conn);
                        }
                        Err(e) => {
                            tracing::warn!("failed to connect to MIDI port {}: {}", name, e);
                            return None;
                        }
                    }
                }
            }
        }

        tracing::info!("MIDI port '{}' not found — MIDI output disabled", PORT_NAME);
        None
    }

    /// Send subdivision as CC. Maps 5 discrete values to CC range.
    /// 0.25 (1/16) → 0, 0.5 (1/8) → 32, 1.0 (1/4) → 64, 2.0 (1/2) → 96, 4.0 (1 measure) → 127
    pub fn send_subdivision(&mut self, beats: f32) {
        let val = match beats as u32 {
            0 => 0,    // 0.25 truncates to 0
            1 => 64,   // 1.0
            2 => 96,   // 2.0
            4 => 127,  // 4.0
            _ => {
                // 0.5 truncates to 0 too, disambiguate
                if beats < 0.4 { 0 } else { 32 }
            }
        };
        self.send_cc(CC_SUBDIVISION, val, &mut self.last_subdivision.clone());
    }

    /// Send feedback as CC (0..1 → 0..127).
    pub fn send_feedback(&mut self, feedback: f32) {
        let val = (feedback * 127.0).round() as u8;
        self.send_cc(CC_FEEDBACK, val, &mut self.last_feedback.clone());
    }

    fn send_cc(&mut self, cc: u8, val: u8, last: &mut u8) {
        if val == *last {
            return;
        }
        if let Some(conn) = &mut self.conn {
            let msg = [0xB0 | MIDI_CHANNEL, cc, val];
            if let Err(e) = conn.send(&msg) {
                tracing::warn!("MIDI send failed: {}", e);
            }
        }
        // Update the stored last value
        match cc {
            CC_SUBDIVISION => self.last_subdivision = val,
            CC_FEEDBACK => self.last_feedback = val,
            _ => {}
        }
    }
}
