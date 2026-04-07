use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, info, warn};

use crate::{
    device::Device,
    event::CoreEvent,
    plugin_interface::Plugin,
    protocol::{PacketType, ProtocolPacket},
};

// X11 keysyms
const XK_BACKSPACE: i32 = 0xFF08;
const XK_TAB: i32 = 0xFF09;
const XK_RETURN: i32 = 0xFF0D;
const XK_RETURN_KP: i32 = 0xFF8D;
const XK_DELETE: i32 = 0xFFFF;
const XK_ESCAPE: i32 = 0xFF1B;
const XK_HOME: i32 = 0xFF50;
const XK_LEFT: i32 = 0xFF51;
const XK_UP: i32 = 0xFF52;
const XK_RIGHT: i32 = 0xFF53;
const XK_DOWN: i32 = 0xFF54;
const XK_PAGE_UP: i32 = 0xFF55;
const XK_PAGE_DOWN: i32 = 0xFF56;
const XK_END: i32 = 0xFF57;
const XK_INSERT: i32 = 0xFF63;
const XK_NUM_LOCK: i32 = 0xFF7F;
const XK_F1: i32 = 0xFFBE;
const XK_SHIFT_L: i32 = 0xFFE1;
const XK_CTRL_L: i32 = 0xFFE3;
const XK_ALT_L: i32 = 0xFFE9;
const XK_SUPER_L: i32 = 0xFFEB;


// Linux input button codes
const BTN_LEFT: i32 = 0x110;
const BTN_RIGHT: i32 = 0x111;
const BTN_MIDDLE: i32 = 0x112;

// ---------- Protocol structs ----------

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct KeyboardState {
    pub state: Option<bool>,
}

impl Plugin for KeyboardState {
    fn id(&self) -> &'static str {
        "kdeconnect.mousepad.keyboardstate"
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct MousePadRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(rename = "specialKey", skip_serializing_if = "Option::is_none")]
    pub special_key: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alt: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ctrl: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shift: Option<bool>,
    #[serde(rename = "super", skip_serializing_if = "Option::is_none")]
    pub super_key: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub singleclick: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doubleclick: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub middleclick: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rightclick: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub singlehold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub singlerelease: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dx: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dy: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scroll: Option<bool>,
    #[serde(rename = "sendAck", skip_serializing_if = "Option::is_none")]
    pub send_ack: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MousePadEcho {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(rename = "specialKey", skip_serializing_if = "Option::is_none")]
    pub special_key: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alt: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ctrl: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shift: Option<bool>,
    #[serde(rename = "isAck")]
    pub is_ack: bool,
}

pub struct MousePadPlugin;

impl Plugin for MousePadPlugin {
    fn id(&self) -> &'static str {
        "mousepad"
    }
}

// ---------- Input backend ----------

#[derive(Debug)]
enum InputEvent {
    PointerMotion { dx: f64, dy: f64 },
    PointerButton { button: i32, pressed: bool },
    PointerScroll { dx: f64, dy: f64 },
    KeySym { keysym: i32, pressed: bool },
}

static INPUT_TX: OnceLock<Mutex<Option<mpsc::UnboundedSender<InputEvent>>>> = OnceLock::new();
static PORTAL_SPAWNED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn input_tx_store() -> &'static Mutex<Option<mpsc::UnboundedSender<InputEvent>>> {
    INPUT_TX.get_or_init(|| Mutex::new(None))
}

pub async fn ensure_remote_desktop() {
    // Fast path — portal already ready.
    {
        let guard = input_tx_store().lock().await;
        if guard.is_some() {
            return;
        }
    }

    // Only spawn one setup task at a time.
    if PORTAL_SPAWNED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }

    info!("MousePad: spawning RemoteDesktop task");
    tokio::spawn(async move {
        use std::sync::atomic::Ordering;
        use ashpd::desktop::remote_desktop::{KeyState, RemoteDesktop};
        use tokio::time::{Duration, timeout};

        macro_rules! bail {
            ($msg:literal, $e:expr) => {{
                warn!($msg, $e);
                PORTAL_SPAWNED.store(false, Ordering::SeqCst);
                return;
            }};
            ($msg:literal) => {{
                warn!($msg);
                PORTAL_SPAWNED.store(false, Ordering::SeqCst);
                return;
            }};
        }

        info!("MousePad: task started, calling RemoteDesktop::new()");
        let rd = match timeout(Duration::from_secs(10), RemoteDesktop::new()).await {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => bail!("MousePad: RemoteDesktop::new() failed: {}", e),
            Err(_) => bail!("MousePad: RemoteDesktop::new() timed out"),
        };

        info!("MousePad: calling create_session()");
        let session = match timeout(Duration::from_secs(10), rd.create_session(Default::default())).await {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => bail!("MousePad: create_session failed: {}", e),
            Err(_) => bail!("MousePad: create_session timed out"),
        };

        info!("MousePad: calling select_devices()");
        match timeout(Duration::from_secs(10), rd.select_devices(&session, Default::default())).await {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => bail!("MousePad: select_devices failed: {}", e),
            Err(_) => bail!("MousePad: select_devices timed out"),
        }

        // start() shows the permission dialog — allow up to 60 s for user response.
        info!("MousePad: calling start() — permission dialog may appear");
        match timeout(Duration::from_secs(60), rd.start(&session, None, Default::default())).await {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => bail!("MousePad: start failed: {}", e),
            Err(_) => bail!("MousePad: start timed out"),
        }

        info!("MousePad: RemoteDesktop session ready");

        // Only publish the sender now that the session is confirmed live.
        let (tx, mut rx) = mpsc::unbounded_channel::<InputEvent>();
        {
            let mut guard = input_tx_store().lock().await;
            *guard = Some(tx);
        }

        while let Some(event) = rx.recv().await {
            let result = match event {
                InputEvent::PointerMotion { dx, dy } => {
                    rd.notify_pointer_motion(&session, dx, dy, Default::default()).await
                }
                InputEvent::PointerButton { button, pressed } => {
                    let state = if pressed { KeyState::Pressed } else { KeyState::Released };
                    rd.notify_pointer_button(&session, button, state, Default::default()).await
                }
                InputEvent::PointerScroll { dx, dy } => {
                    rd.notify_pointer_axis(&session, dx, dy, Default::default()).await
                }
                InputEvent::KeySym { keysym, pressed } => {
                    let state = if pressed { KeyState::Pressed } else { KeyState::Released };
                    rd.notify_keyboard_keysym(&session, keysym, state, Default::default()).await
                }
            };
            if let Err(e) = result {
                debug!("MousePad: input event error: {}", e);
            }
        }

        // Channel closed — reset so the next mouse session can retry.
        info!("MousePad: input channel closed, resetting state");
        {
            let mut guard = input_tx_store().lock().await;
            *guard = None;
        }
        PORTAL_SPAWNED.store(false, Ordering::SeqCst);
    });
}

async fn send_input(event: InputEvent) {
    let guard = input_tx_store().lock().await;
    if let Some(tx) = guard.as_ref() {
        let _ = tx.send(event);
    }
}

pub fn special_key_to_keysym(code: u32) -> Option<i32> {
    match code {
        1 => Some(XK_BACKSPACE),
        2 => Some(XK_TAB),
        3 => Some(XK_RETURN_KP),
        4 => Some(XK_RETURN),
        5 => Some(XK_DELETE),
        6 => Some(XK_ESCAPE),
        9..=20 => Some(XK_F1 + (code - 9) as i32),
        21 => Some(XK_LEFT),
        22 => Some(XK_UP),
        23 => Some(XK_RIGHT),
        24 => Some(XK_DOWN),
        25 => Some(XK_PAGE_UP),
        26 => Some(XK_PAGE_DOWN),
        27 => Some(XK_HOME),
        28 => Some(XK_END),
        29 => Some(XK_RETURN),
        30 => Some(XK_INSERT),
        31 => Some(XK_NUM_LOCK),
        _ => None,
    }
}

// ---------- Packet handling ----------

impl MousePadRequest {
    pub async fn received_packet(
        &self,
        device: &Device,
        core_tx: &mpsc::UnboundedSender<CoreEvent>,
    ) {
        info!("MousePad: calling ensure_remote_desktop");
        ensure_remote_desktop().await;
        info!("MousePad: ensure_remote_desktop returned");

        // Mouse movement / scroll
        if let (Some(dx), Some(dy)) = (self.dx, self.dy) {
            if self.scroll == Some(true) {
                send_input(InputEvent::PointerScroll { dx, dy }).await;
            } else {
                send_input(InputEvent::PointerMotion { dx, dy }).await;
            }
        }

        // Mouse clicks
        if self.singlehold == Some(true) {
            send_input(InputEvent::PointerButton { button: BTN_LEFT, pressed: true }).await;
        } else if self.singlerelease == Some(true) {
            send_input(InputEvent::PointerButton { button: BTN_LEFT, pressed: false }).await;
        } else if self.singleclick == Some(true) {
            send_input(InputEvent::PointerButton { button: BTN_LEFT, pressed: true }).await;
            send_input(InputEvent::PointerButton { button: BTN_LEFT, pressed: false }).await;
        } else if self.doubleclick == Some(true) {
            for _ in 0..2 {
                send_input(InputEvent::PointerButton { button: BTN_LEFT, pressed: true }).await;
                send_input(InputEvent::PointerButton { button: BTN_LEFT, pressed: false }).await;
            }
        } else if self.rightclick == Some(true) {
            send_input(InputEvent::PointerButton { button: BTN_RIGHT, pressed: true }).await;
            send_input(InputEvent::PointerButton { button: BTN_RIGHT, pressed: false }).await;
        } else if self.middleclick == Some(true) {
            send_input(InputEvent::PointerButton { button: BTN_MIDDLE, pressed: true }).await;
            send_input(InputEvent::PointerButton { button: BTN_MIDDLE, pressed: false }).await;
        }

        // Keyboard
        let keysym = if let Some(ref key) = self.key {
            key.chars().next().map(|c| {
                let cp = c as i32;
                if cp > 0x7F { 0x01000000 | cp } else { cp }
            })
        } else if let Some(code) = self.special_key {
            special_key_to_keysym(code)
        } else {
            None
        };

        if let Some(ks) = keysym {
            if self.ctrl == Some(true) {
                send_input(InputEvent::KeySym { keysym: XK_CTRL_L, pressed: true }).await;
            }
            if self.alt == Some(true) {
                send_input(InputEvent::KeySym { keysym: XK_ALT_L, pressed: true }).await;
            }
            if self.shift == Some(true) {
                send_input(InputEvent::KeySym { keysym: XK_SHIFT_L, pressed: true }).await;
            }
            if self.super_key == Some(true) {
                send_input(InputEvent::KeySym { keysym: XK_SUPER_L, pressed: true }).await;
            }

            send_input(InputEvent::KeySym { keysym: ks, pressed: true }).await;
            send_input(InputEvent::KeySym { keysym: ks, pressed: false }).await;

            if self.super_key == Some(true) {
                send_input(InputEvent::KeySym { keysym: XK_SUPER_L, pressed: false }).await;
            }
            if self.shift == Some(true) {
                send_input(InputEvent::KeySym { keysym: XK_SHIFT_L, pressed: false }).await;
            }
            if self.alt == Some(true) {
                send_input(InputEvent::KeySym { keysym: XK_ALT_L, pressed: false }).await;
            }
            if self.ctrl == Some(true) {
                send_input(InputEvent::KeySym { keysym: XK_CTRL_L, pressed: false }).await;
            }
        }

        // Echo / ack
        if self.send_ack == Some(true) {
            let echo = MousePadEcho {
                key: self.key.clone(),
                special_key: self.special_key,
                alt: self.alt,
                ctrl: self.ctrl,
                shift: self.shift,
                is_ack: true,
            };
            if let Ok(body) = serde_json::to_value(echo) {
                let pkt = ProtocolPacket::new(PacketType::MousePadEcho, body);
                let _ = core_tx.send(CoreEvent::SendPacket {
                    device: device.device_id.clone(),
                    packet: pkt,
                });
            }
        }
    }
}
