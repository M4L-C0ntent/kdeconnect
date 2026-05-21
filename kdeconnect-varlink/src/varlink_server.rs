//! Varlink server — runs alongside D-Bus, handles applet/settings IPC via Unix socket.

use anyhow::Result;
use async_trait::async_trait;
use kdeconnect_varlink::iface::{
    self, BatteryState, Device, VarlinkInterface,
    Call_ListDevices, Call_PairDevice, Call_UnpairDevice, Call_SendPing,
    Call_SendFiles, Call_SendClipboard, Call_RunCommand,
    Call_SetPluginEnabled, Call_GetPluginEnabled,
    Call_AcceptPairing, Call_RejectPairing, Call_Subscribe,
};
use kdeconnect_varlink::socket_address;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use varlink::{listen_async, AsyncVarlinkService, ListenAsyncConfig};

use crate::app_event::AppEvent;
use kdeconnect_core::{PacketType, ProtocolPacket, device::DeviceId, event::AppEvent};
use serde_json::json;

#[derive(Debug, Clone)]
pub struct VarlinkEvent {
    pub event_type: String,
    pub device_id: String,
    pub device: Option<DbusDevice>,
    pub battery: Option<(i32, bool)>,
    pub connectivity_strength: Option<i32>,
    pub clipboard_content: Option<String>,
    pub commands_json: Option<String>,
}

pub struct KdeConnectVarlinkService {
    event_sender: Arc<mpsc::UnboundedSender<AppEvent>>,
    devices: Arc<tokio::sync::Mutex<std::collections::HashMap<String, DbusDevice>>>,
    broadcast_tx: broadcast::Sender<VarlinkEvent>,
}

impl KdeConnectVarlinkService {
    pub fn new(
        event_sender: Arc<mpsc::UnboundedSender<AppEvent>>,
        devices: Arc<tokio::sync::Mutex<std::collections::HashMap<String, DbusDevice>>>,
        broadcast_tx: broadcast::Sender<VarlinkEvent>,
    ) -> Self {
        Self { event_sender, devices, broadcast_tx }
    }
}

fn to_varlink_device(d: &DbusDevice) -> Device {
    Device {
        id: d.id.clone(),
        name: d.name.clone(),
        device_type: d.device_type.clone(),
        is_paired: d.is_paired,
        is_reachable: d.is_reachable,
    }
}

#[async_trait]
impl VarlinkInterface for KdeConnectVarlinkService {
    async fn list_devices(&self, call: &mut dyn Call_ListDevices) -> varlink::Result<()> {
        let guard = self.devices.lock().await;
        let devices: Vec<Device> = guard.values().map(to_varlink_device).collect();
        call.reply(devices)
    }

    async fn pair_device(&self, call: &mut dyn Call_PairDevice, device_id: String) -> varlink::Result<()> {
        let _ = self.event_sender.send(AppEvent::Pair(DeviceId(device_id)));
        call.reply()
    }

    async fn unpair_device(&self, call: &mut dyn Call_UnpairDevice, device_id: String) -> varlink::Result<()> {
        let _ = self.event_sender.send(AppEvent::Unpair(DeviceId(device_id)));
        call.reply()
    }

    async fn send_ping(&self, call: &mut dyn Call_SendPing, device_id: String, message: String) -> varlink::Result<()> {
        let _ = self.event_sender.send(AppEvent::Ping((DeviceId(device_id), message)));
        call.reply()
    }

    async fn send_files(&self, call: &mut dyn Call_SendFiles, device_id: String, files: Vec<String>) -> varlink::Result<()> {
        let _ = self.event_sender.send(AppEvent::SendFiles(DeviceId(device_id), files));
        call.reply()
    }

    async fn send_clipboard(&self, call: &mut dyn Call_SendClipboard, device_id: String, content: String) -> varlink::Result<()> {
        let _ = self.event_sender.send(AppEvent::SendClipboard(DeviceId(device_id), content));
        call.reply()
    }

    async fn run_command(&self, call: &mut dyn Call_RunCommand, device_id: String, key: String) -> varlink::Result<()> {
        use kdeconnect_core::protocol::{PacketType, ProtocolPacket};
        let packet = ProtocolPacket::new(PacketType::RunCommandRequest, serde_json::json!({ "key": key }));
        let _ = self.event_sender.send(AppEvent::SendPacket(DeviceId(device_id), packet));
        call.reply()
    }

    async fn set_plugin_enabled(
        &self, call: &mut dyn Call_SetPluginEnabled,
        device_id: String, plugin: String, enabled: bool,
    ) -> varlink::Result<()> {
        let _ = self.event_sender.send(AppEvent::SetPluginEnabled(DeviceId(device_id), plugin, enabled));
        call.reply()
    }

    async fn get_plugin_enabled(
        &self, call: &mut dyn Call_GetPluginEnabled,
        device_id: String, plugin: String,
    ) -> varlink::Result<()> {
        let _ = self.event_sender.send(AppEvent::GetPluginEnabled(DeviceId(device_id.clone()), plugin.clone()));
        // Placeholder — proper async response requires a oneshot channel wired into AppEvent
        call.reply(true)
    }

    async fn accept_pairing(&self, call: &mut dyn Call_AcceptPairing, device_id: String) -> varlink::Result<()> {
        let _ = self.event_sender.send(AppEvent::AcceptPair(DeviceId(device_id)));
        call.reply()
    }

    async fn reject_pairing(&self, call: &mut dyn Call_RejectPairing, device_id: String) -> varlink::Result<()> {
        let _ = self.event_sender.send(AppEvent::RejectPair(DeviceId(device_id)));
        call.reply()
    }

    async fn subscribe(&self, call: &mut dyn Call_Subscribe) -> varlink::Result<()> {
        let mut rx = self.broadcast_tx.subscribe();
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    let device = ev.device.as_ref().map(to_varlink_device);
                    let battery = ev.battery.map(|(level, is_charging)| BatteryState { level, is_charging });
                    call.reply(
                        ev.event_type,
                        ev.device_id,
                        device,
                        battery,
                        ev.connectivity_strength,
                        ev.clipboard_content,
                        ev.commands_json,
                        None,
                    )?;
                }
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
        Ok(())
    }
}

pub async fn run_varlink_server(
    event_sender: Arc<mpsc::UnboundedSender<AppEvent>>,
    devices: Arc<tokio::sync::Mutex<std::collections::HashMap<String, DbusDevice>>>,
    broadcast_tx: broadcast::Sender<VarlinkEvent>,
) -> Result<()> {
    let service = Arc::new(KdeConnectVarlinkService::new(event_sender, devices, broadcast_tx));
    let handler = Arc::new(iface::new(service));

    let varlink_service = Arc::new(AsyncVarlinkService::new(
        "io.github.hepp3n",
        "KDE Connect",
        env!("CARGO_PKG_VERSION"),
        "https://github.com/hepp3n/kdeconnect",
        vec![handler],
    ));

    listen_async(
        varlink_service,
        &socket_address(),
        &ListenAsyncConfig::default(),
    )
    .await?;

    Ok(())
}
