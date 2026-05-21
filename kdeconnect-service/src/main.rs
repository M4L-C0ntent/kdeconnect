//! KDE Connect D-Bus Service Daemon

use anyhow::Result;
use tokio::sync::broadcast;
use tracing::{error, info};

mod dbus_interface;
mod varlink_server;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    info!("KDE Connect service starting");

    {
        let guard_conn = zbus::Connection::session().await?;
        match guard_conn
            .request_name_with_flags(
                "io.github.hepp3n.kdeconnect",
                zbus::fdo::RequestNameFlags::DoNotQueue.into(),
            )
            .await
        {
            Ok(zbus::fdo::RequestNameReply::PrimaryOwner) => {
                info!("Single-instance guard passed");
            }
            Ok(_) => {
                info!("Another instance is already running — exiting");
                return Ok(());
            }
            Err(e) => {
                return Err(e.into());
            }
        }
    }

    let service = dbus_interface::KdeConnectService::new().await?;
    info!("D-Bus service started on io.github.hepp3n.kdeconnect");

    // Spawn varlink server alongside D-Bus.
    // broadcast channel capacity of 64 is enough for burst events from a single phone.
    let (broadcast_tx, _) = broadcast::channel(64);
    service.start_varlink(broadcast_tx);

    service.run().await?;

    std::process::exit(0);
}
