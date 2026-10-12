//! All mouse access runs in a background thread; the window never blocks.
//! A job returns a function that updates the state in the GUI thread.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use m6980x_core::fake::FakeMouse;
use m6980x_core::{M6980X, Result, Transport, open_transport};

use crate::app::App;

pub type G = M6980X<Box<dyn Transport>>;
pub type Apply = Box<dyn FnOnce(&mut App) + Send>;
pub type Job = Box<dyn FnOnce(&mut G) -> Result<Apply> + Send>;

pub struct Request {
    pub label: String,
    pub quiet: bool,
    pub job: Job,
}

pub struct Done {
    pub label: String,
    pub quiet: bool,
    pub result: std::result::Result<Apply, String>,
    /// the mouse is open after the job (an error closes it until the next job)
    pub connected: bool,
}

/// Simulated mouse with an empty memory (like a freshly erased mouse) for the dry run without a device.
fn fake_mouse() -> Box<dyn Transport> {
    let mut m = FakeMouse::new();
    m.rom[..0x1F00].fill(0xFF);
    Box::new(m)
}

pub struct Device {
    tx: Sender<Request>,
    pub rx: Receiver<Done>,
    /// queued + running, only used in the GUI thread
    pub pending: usize,
}

impl Device {
    pub fn spawn(ctx: eframe::egui::Context, device: Option<String>, fake: bool) -> Self {
        let (tx, jobs) = channel::<Request>();
        let (done_tx, rx) = channel::<Done>();
        thread::spawn(move || {
            let mut g: Option<G> = None;
            for req in jobs {
                let result = (|| {
                    if g.is_none() {
                        let t = if fake { fake_mouse() } else { open_transport(device.as_deref())? };
                        let mut m = M6980X::new(t);
                        m.check_model()?; // only the memory layout of the 6980X is known
                        g = Some(m);
                    }
                    (req.job)(g.as_mut().expect("connected"))
                })();
                if result.is_err() && !fake {
                    g = None; // reconnect on the next job
                }
                let done = Done {
                    label: req.label,
                    quiet: req.quiet,
                    result: result.map_err(|e| e.to_string()),
                    connected: g.is_some(),
                };
                if done_tx.send(done).is_err() {
                    break;
                }
                ctx.request_repaint();
            }
        });
        Self { tx, rx, pending: 0 }
    }

    pub fn submit(&mut self, label: String, quiet: bool, job: Job) {
        self.pending += 1;
        // The worker lives as long as the window; an error here would mean it crashed.
        if self.tx.send(Request { label, quiet, job }).is_err() {
            self.pending -= 1;
        }
    }

    pub fn idle(&self) -> bool {
        self.pending == 0
    }
}
