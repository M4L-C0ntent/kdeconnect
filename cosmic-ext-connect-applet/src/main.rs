#[macro_use]
extern crate cosmic_ext_connect_applet;

use cosmic_ext_connect_applet::{backend, messages, models, portal, ui};

use messages::Message;
use models::{Device, NowPlaying};

use cosmic::app::Core;
use cosmic::iced::window::Id as SurfaceId;
use cosmic::iced::{Limits, Subscription};
use cosmic::iced::platform_specific::shell::commands::popup::{destroy_popup, get_popup};
use cosmic::{Element, Task, widget};
use std::collections::HashMap;
use cosmic_ext_connect_applet::theme;
use tracing::{debug, error, info};

pub struct KdeConnectApplet {
    core: Core,
    popup: Option<SurfaceId>,
    devices: HashMap<String, Device>,
    expanded_device: Option<String>,
    /// Pending pairing requests: device_id → device_name
    pairing_requests: HashMap<String, String>,
    accent_color: cosmic::iced::Color,
    /// device_id -> has unread SMS, for the quick-actions menu indicator.
    unread_sms: HashMap<String, bool>,
    /// Set when an action fails in a way the user needs to know about and
    /// can act on (e.g. browse-device preflight checks); shown as a
    /// dismissible banner in the popup.
    error_banner: Option<String>,
    /// Media section state, keyed by MPRIS D-Bus bus name. Refreshed by
    /// `backend::mpris_subscription`.
    now_playing: HashMap<String, NowPlaying>,
}

impl KdeConnectApplet {
    /// Creates a new popup surface (tracked in `self.popup`) with the
    /// applet's standard size limits. Shared by TogglePopup and
    /// PairingRequestReceived, which both need to open the popup.
    fn show_popup(&mut self) -> Task<cosmic::Action<Message>> {
        let new_id = SurfaceId::unique();
        self.popup.replace(new_id);

        let mut popup_settings = self.core.applet.get_popup_settings(
            self.core.main_window_id().unwrap(),
            new_id,
            None,
            None,
            None,
        );
        popup_settings.positioner.size_limits = Limits::NONE
            .max_width(400.0)
            .min_width(300.0)
            .min_height(200.0)
            .max_height(600.0);

        get_popup(popup_settings)
    }
}

impl cosmic::Application for KdeConnectApplet {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = "io.github.hepp3n.kdeconnect";

    fn core(&self) -> &Core {
        &self.core
    }
    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<cosmic::Action<Self::Message>>) {
        tokio::spawn(async {
            if let Err(e) = backend::initialize().await {
                error!("Backend init failed: {:?}", e);
            }
        });

        let app = KdeConnectApplet {
            core,
            popup: None,
            devices: HashMap::new(),
            expanded_device: None,
            pairing_requests: HashMap::new(),
            accent_color: theme::try_load_cosmic_accent()
                .unwrap_or(theme::FALLBACK_TEAL),
            unread_sms: HashMap::new(),
            error_banner: None,
            now_playing: HashMap::new(),
        };

        (app, Task::none())
    }

    fn on_close_requested(&self, id: SurfaceId) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {        match message {
            Message::TogglePopup => {
                self.accent_color = theme::try_load_cosmic_accent()
                    .unwrap_or(theme::FALLBACK_TEAL);
                return if let Some(p) = self.popup.take() {
                    destroy_popup(p)
                } else {
                    Task::batch(vec![
                        self.show_popup(),
                        Task::perform(backend::fetch_devices(), |devices| {
                            cosmic::Action::App(Message::DevicesUpdated(devices))
                        }),
                    ])
                };
            }
            Message::PopupClosed(id) => {
                if self.popup == Some(id) {
                    self.popup = None;
                }
            }
            Message::RefreshDevices => {
                let device_ids: Vec<String> = self.devices.keys().cloned().collect();
                return Task::batch(vec![
                    Task::perform(backend::fetch_devices(), |devices| {
                        cosmic::Action::App(Message::DevicesUpdated(devices))
                    }),
                    Task::perform(backend::check_unread_sms(device_ids), |unread| {
                        cosmic::Action::App(Message::UnreadSmsUpdated(unread))
                    }),
                ]);
            }
            Message::UnreadSmsUpdated(unread) => {
                self.unread_sms = unread;
            }
            Message::DevicesUpdated(devices) => {
                self.devices.clear();
                for device in devices {
                    self.devices.insert(device.id.clone(), device);
                }
            }
            Message::DelayedRefresh => {
                return Task::perform(backend::fetch_devices(), |devices| {
                    cosmic::Action::App(Message::DevicesUpdated(devices))
                });
            }
            Message::ToggleDeviceMenu(ref device_id) => {
                if self.expanded_device.as_ref() == Some(device_id) {
                    self.expanded_device = None;
                } else {
                    self.expanded_device = Some(device_id.clone());
                    let id = device_id.clone();
                    return Task::perform(
                        async move { backend::request_run_commands(id).await.ok(); },
                        |_| cosmic::Action::App(Message::RefreshDevices),
                    );
                }
            }
            Message::SendSMS(ref device_id) => {
                // Look up device name for the window title
                let device_name = self
                    .devices
                    .get(device_id)
                    .map(|d| d.name.clone())
                    .unwrap_or_else(|| "Unknown Device".to_string());
                let id = device_id.clone();

                info!(
                    "Launching SMS window for device={} name={}",
                    id, device_name
                );

                // Spawn in a thread so the process::Command doesn't block the executor
                std::thread::spawn(move || {
                    match std::process::Command::new("cosmic-ext-connect-sms")
                        .arg(&id)
                        .arg(&device_name)
                        .spawn()
                    {
                        Ok(_) => info!("cosmic-ext-connect-sms launched"),
                        Err(e) => error!("Failed to launch cosmic-ext-connect-sms: {:?}", e),
                    }
                });
            }
            Message::PingDevice(ref device_id) => {
                let id = device_id.clone();
                return Task::perform(
                    async move {
                        backend::ping_device(id).await.ok();
                    },
                    |_| cosmic::Action::App(Message::RefreshDevices),
                );
            }
            Message::RingDevice(ref device_id) => {
                let id = device_id.clone();
                return Task::perform(
                    async move {
                        backend::ring_device(id).await.ok();
                    },
                    |_| cosmic::Action::App(Message::RefreshDevices),
                );
            }
            Message::BrowseDevice(ref device_id) => {
                let id = device_id.clone();
                return Task::perform(
                    async move { backend::browse_device_filesystem(id).await },
                    |result| match result {
                        Ok(()) => cosmic::Action::App(Message::RefreshDevices),
                        Err(e) => cosmic::Action::App(Message::BrowseDeviceFailed(e.to_string())),
                    },
                );
            }
            Message::BrowseDeviceFailed(message) => {
                self.error_banner = Some(message);
                return Task::none();
            }
            Message::DismissError => {
                self.error_banner = None;
                return Task::none();
            }
            Message::PairDevice(ref device_id) => {
                let id = device_id.clone();
                return Task::perform(
                    async move {
                        backend::pair_device(id).await.ok();
                    },
                    |_| cosmic::Action::App(Message::RefreshDevices),
                );
            }
            Message::UnpairDevice(ref device_id) => {
                let id = device_id.clone();
                return Task::perform(
                    async move {
                        backend::unpair_device(id).await.ok();
                    },
                    |_| cosmic::Action::App(Message::RefreshDevices),
                );
            }
            Message::SendFiles(ref device_id) => {
                let id = device_id.clone();
                return Task::perform(
                    async move {
                        let files = portal::pick_files(&fl!("file-picker-title"), true, None).await;
                        if !files.is_empty() {
                            backend::send_files(id, files).await.ok();
                        }
                    },
                    |_| cosmic::Action::App(Message::RefreshDevices),
                );
            }
            Message::UpdateTransferProgress(progress) => {
                if let Some(ref current_device) = self.expanded_device {
                    if let Some(device) = self.devices.get_mut(current_device) {
                        device.share_progress = if progress < 100 { Some(progress) } else { None };
                    }
                }
            }
            Message::ShareClipboard(ref device_id) => {
                let id = device_id.clone();
                return cosmic::iced::clipboard::read().map(move |content| {
                    cosmic::Action::App(Message::ClipboardReadForDevice(
                        id.clone(),
                        content.unwrap_or_default(),
                    ))
                });
            }
            Message::ClipboardReadForDevice(device_id, content) => {
                if !content.is_empty() {
                    return Task::perform(
                        async move { backend::send_clipboard(device_id, content).await.ok(); },
                        |_| cosmic::Action::App(Message::RefreshDevices),
                    );
                }
            }
            Message::ClipboardReceived(content) => {
                return cosmic::iced::clipboard::write::<cosmic::Action<Message>>(content);
            }
            Message::BatteryUpdated(device_id, level, charging) => {
                if let Some(device) = self.devices.get_mut(&device_id) {
                    device.battery_level = Some(level);
                    device.is_charging = Some(charging);
                    // Also patch the backend cache so the next fetch_devices() preserves it
                    let d = device.clone();
                    tokio::spawn(async move { backend::update_device(device_id, d).await; });
                }
            }
            Message::ConnectivityUpdated(device_id, strength) => {
                if let Some(device) = self.devices.get_mut(&device_id) {
                    device.signal_strength = Some(strength);
                    let d = device.clone();
                    tokio::spawn(async move { backend::update_device(device_id, d).await; });
                }
            }
            Message::AcceptPairing(ref device_id) => {
                self.pairing_requests.remove(device_id);
                let id = device_id.clone();
                return Task::perform(
                    async move {
                        backend::accept_pairing(id).await.ok();
                    },
                    |_| cosmic::Action::App(Message::RefreshDevices),
                );
            }
            Message::RejectPairing(ref device_id) => {
                self.pairing_requests.remove(device_id);
                let id = device_id.clone();
                return Task::perform(
                    async move {
                        backend::reject_pairing(id).await.ok();
                    },
                    |_| cosmic::Action::App(Message::RefreshDevices),
                );
            }
            Message::PairingRequestReceived(device_id, device_name) => {
                info!("Pairing request received from {} ({})", device_name, device_id);
                self.pairing_requests.insert(device_id, device_name.clone());

                // Show a system notification so the user is alerted even if they
                // are not looking at the panel. COSMIC's daemon doesn't support
                // action buttons so we just point them to the applet.
                let notif_body = format!(
                    "'{}' wants to pair with this device. Click the KDE Connect applet to accept or decline.",
                    device_name
                );
                tokio::task::spawn_blocking(move || {
                    let _ = notify_rust::Notification::new()
                        .appname("KDE Connect")
                        .summary(&fl!("notification-pairing-summary"))
                        .body(&notif_body)
                        .icon("network-wireless-symbolic")
                        .show();
                });

                // Ensure popup is open so the user sees Accept/Decline immediately.
                if self.popup.is_none() {
                    return self.show_popup();
                }
            }
            Message::MprisReceived(device_id, mpris_data) => {
                debug!("MPRIS from {}: {:?}", device_id, mpris_data);
            }
            Message::MprisSnapshot(snapshot) => {
                self.now_playing = snapshot;
            }
            Message::MprisPlayPause(ref bus_name) => {
                let bus_name = bus_name.clone();
                return Task::perform(
                    backend::mpris_control(bus_name, backend::MprisControlAction::PlayPause),
                    |_| cosmic::Action::App(Message::RefreshDevices),
                );
            }
            Message::MprisNext(ref bus_name) => {
                let bus_name = bus_name.clone();
                return Task::perform(
                    backend::mpris_control(bus_name, backend::MprisControlAction::Next),
                    |_| cosmic::Action::App(Message::RefreshDevices),
                );
            }
            Message::MprisPrevious(ref bus_name) => {
                let bus_name = bus_name.clone();
                return Task::perform(
                    backend::mpris_control(bus_name, backend::MprisControlAction::Previous),
                    |_| cosmic::Action::App(Message::RefreshDevices),
                );
            }
            Message::OpenSettings => {
                std::process::Command::new("cosmic-ext-connect-settings")
                    .spawn()
                    .ok();
            }
            Message::RemoteInput(ref device_id) => {
                debug!("Remote input: {}", device_id);
            }
            Message::LockDevice(ref device_id) => {
                debug!("Lock device: {}", device_id);
            }
            Message::PresenterMode(ref device_id) => {
                debug!("Presenter mode: {}", device_id);
            }
            Message::UseAsMonitor(ref device_id) => {
                debug!("Use as monitor: {}", device_id);
            }
            Message::ShareText(ref device_id) => {
                debug!("Share text: {}", device_id);
            }
            Message::ShareUrl(ref device_id) => {
                debug!("Share URL: {}", device_id);
            }
            Message::RequestRunCommands(ref device_id) => {
                let id = device_id.clone();
                return Task::perform(
                    async move { backend::request_run_commands(id).await.ok(); },
                    |_| cosmic::Action::App(Message::RefreshDevices),
                );
            }
            Message::RunCommandsReceived(ref device_id, ref commands_json) => {
                let commands: Vec<(String, String)> =
                    serde_json::from_str::<Vec<serde_json::Value>>(commands_json)
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|v| {
                            let key = v["key"].as_str()?.to_string();
                            let name = v["name"].as_str()?.to_string();
                            Some((key, name))
                        })
                        .collect();
                if let Some(device) = self.devices.get_mut(device_id) {
                    device.run_commands = commands;
                    let d = device.clone();
                    let did = device_id.clone();
                    tokio::spawn(async move { backend::update_device(did, d).await; });
                }
            }
            Message::ExecuteRunCommand(ref device_id, ref key) => {
                let id = device_id.clone();
                let k = key.clone();
                return Task::perform(
                    async move { backend::execute_run_command(id, k).await.ok(); },
                    |_| cosmic::Action::App(Message::RefreshDevices),
                );
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Self::Message> {
        self.core
            .applet
            .icon_button("phone-symbolic")
            .on_press(Message::TogglePopup)
            .into()
    }

    fn view_window(&self, id: SurfaceId) -> Element<'_, Self::Message> {
        let Some(popup_id) = self.popup else {
            return widget::text("").into();
        };
        if id != popup_id {
            return widget::text("").into();
        }
        ui::popup::create_popup_view(
            &self.core,
            &self.devices,
            self.expanded_device.as_ref(),
            Some(&self.pairing_requests),
            self.accent_color,
            &self.unread_sms,
            self.error_banner.as_ref(),
            &self.now_playing,
        )
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        use futures::StreamExt as _;
        Subscription::batch(vec![
            cosmic::iced::time::every(std::time::Duration::from_secs(10))
                .map(|_| Message::RefreshDevices),
            backend::filetransfer_subscription(),
            backend::service_watcher_subscription(),
            backend::mpris_subscription(),
            // D-Bus event stream — delivers pairing requests and device state
            // changes in real time without waiting for the 10s poll.
            Subscription::run(|| {
                async_stream::stream! {
                    let mut stream = backend::event_stream().await;
                    while let Some(event) = stream.next().await {
                        match event {
                            kdeconnect_dbus_client::ServiceEvent::PairingRequested(id, name) => {
                                yield Message::PairingRequestReceived(id, name);
                            }
                            kdeconnect_dbus_client::ServiceEvent::ClipboardReceived(content) => {
                                yield Message::ClipboardReceived(content);
                            }
                            kdeconnect_dbus_client::ServiceEvent::BatteryReceived(id, level, charging) => {
                                yield Message::BatteryUpdated(id, level, charging);
                            }
                            kdeconnect_dbus_client::ServiceEvent::ConnectivityReceived(id, strength) => {
                                yield Message::ConnectivityUpdated(id, strength);
                            }
                            kdeconnect_dbus_client::ServiceEvent::RunCommandListReceived(id, commands_json) => {
                                yield Message::RunCommandsReceived(id, commands_json);
                            }
                            kdeconnect_dbus_client::ServiceEvent::BrowseFailed(_id, message) => {
                                yield Message::BrowseDeviceFailed(message);
                            }
                            kdeconnect_dbus_client::ServiceEvent::DeviceConnected(id, _)
                            | kdeconnect_dbus_client::ServiceEvent::DevicePaired(id, _)
                            | kdeconnect_dbus_client::ServiceEvent::DeviceDisconnected(id) => {
                                let _ = id;
                                yield Message::RefreshDevices;
                            }
                            _ => {}
                        }
                    }
                }
            }),
        ])
    }
}

fn main() -> cosmic::iced::Result {
    use tracing_subscriber::prelude::*;

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"));

    let stderr_layer = tracing_subscriber::fmt::layer().with_writer(std::io::stderr);

    if std::env::var("KDECONNECT_LOG_FILE").is_ok_and(|v| !v.is_empty())
        && std::path::Path::new("/.flatpak-info").exists()
    {
        let log_dir = dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
        let _ = std::fs::create_dir_all(&log_dir);
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_dir.join("applet.log"))
            .expect("failed to open applet.log");
        let (non_blocking, _guard) = tracing_appender::non_blocking(file);
        let file_layer = tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(non_blocking);
        tracing_subscriber::registry()
            .with(env_filter)
            .with(stderr_layer)
            .with(file_layer)
            .init();
        std::mem::forget(_guard);
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(stderr_layer)
            .init();
    }

    ctrlc::set_handler(move || std::process::exit(0)).ok();

    // Spawn the service in the same process group so it exits when the session ends.
    // If the service is already running it exits immediately (D-Bus name already taken).
    // Explicitly forward HOME so the service reads config from the correct path
    // regardless of the environment the COSMIC panel provides.
    let home = std::env::var("HOME").unwrap_or_else(|_| {
        dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
            .to_string_lossy()
            .to_string()
    });
    let _ = std::process::Command::new("kdeconnect-service")
        .env("HOME", &home)
        .env("XDG_RUNTIME_DIR", std::env::var("XDG_RUNTIME_DIR").unwrap_or_default())
        .env("XDG_CONFIG_HOME", std::env::var("XDG_CONFIG_HOME").unwrap_or_default())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();

    cosmic::applet::run::<KdeConnectApplet>(())
}
