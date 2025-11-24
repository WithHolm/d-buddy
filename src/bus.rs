use anyhow::Result; // Added for error propagation
use futures::StreamExt; // Required for stream.next().await
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::fs; // For reading /proc/cmdline
use zbus::{fdo::DBusProxy, Connection, MessageStream}; // For parsing app name from path

// ... existing Item struct and impl Default for Item ...
#[derive(Debug, Clone)]
pub struct Item {
    pub timestamp: SystemTime,
    pub sender: String,
    pub receiver: String,
    pub member: String,
    pub path: String,
    pub message: Option<zbus::Message>,
    pub serial: String,
    pub reply_serial: String,
    pub is_reply: bool,
    pub stream_type: BusType,
}
// ... existing BusType and GroupingType enums and impls ...
// what type of bus is this?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BusType {
    Session = 0,
    System = 1,
    Both = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupingType {
    Sender,
    Member,
    Path,
    Serial,
    None,
}

async fn get_process_info(
    conn: &zbus::Connection,
    sender_name: &str,
) -> Option<(u32, String, String, Vec<String>)> {
    let proxy = zbus::fdo::DBusProxy::new(conn).await.ok()?;
    let pid: u32 = proxy
        .get_connection_unix_process_id(sender_name)
        .await
        .ok()?;

    let cmdline_path = format!("/proc/{}/cmdline", pid);
    let cmdline_content = tokio::fs::read(&cmdline_path).await.ok()?;

    // cmdline is null-separated, typically the first entry is the executable path
    let args: Vec<String> = cmdline_content
        .split(|&b| b == 0)
        .filter_map(|s| {
            if s.is_empty() {
                None
            } else {
                String::from_utf8(s.to_vec()).ok()
            }
        })
        .collect();

    if args.is_empty() {
        return Some((pid, "Unknown".to_string(), "".to_string(), Vec::new()));
    }

    let app_path = args[0].clone();
    let app_name = PathBuf::from(&app_path)
        .file_name()?
        .to_string_lossy()
        .to_string();

    Some((pid, app_name, app_path, args))
}

pub async fn dbus_listener(t: BusType) -> Result<Arc<tokio::sync::Mutex<Vec<Item>>>> {
    let messages = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let messages_clone = Arc::clone(&messages);

    let conn: Connection = match t {
        BusType::Session => zbus::Connection::session().await?,
        BusType::System => zbus::Connection::system().await?,
        BusType::Both => zbus::Connection::session().await?,
    };

    let proxy = DBusProxy::new(&conn).await?;
    proxy
        .add_match_rule(
            zbus::MatchRule::builder()
                .msg_type(zbus::message::Type::Signal)
                .build(),
        )
        .await?;
    proxy
        .add_match_rule(
            zbus::MatchRule::builder()
                .msg_type(zbus::message::Type::MethodCall)
                .build(),
        )
        .await?;
    proxy
        .add_match_rule(
            zbus::MatchRule::builder()
                .msg_type(zbus::message::Type::MethodReturn)
                .build(),
        )
        .await?;
    proxy
        .add_match_rule(
            zbus::MatchRule::builder()
                .msg_type(zbus::message::Type::Error)
                .build(),
        )
        .await?;

    let stream = MessageStream::from(&conn);
    tokio::spawn(async move {
        let mut stream = stream;
        while let Some(Ok(msg)) = stream.next().await {
            let header = msg.header();
            let sender_name = header
                .sender()
                .map(|s| s.as_str().to_string())
                .unwrap_or_default();

            let mut pid_val = None;
            let mut app_name_val = "Unknown".to_string();
            let mut app_path_val = String::new();
            let mut app_args_val = Vec::new();

            if sender_name.starts_with(':') {
                if let Some((p, an, ap, aa)) = get_process_info(&conn, &sender_name).await {
                    pid_val = Some(p);
                    app_name_val = an;
                    app_path_val = ap;
                    app_args_val = aa;
                }
            }

            let item = Item {
                timestamp: SystemTime::now(),
                sender: sender_name,
                receiver: header
                    .destination()
                    .map(|s| s.as_str().to_string())
                    .unwrap_or_default(),
                member: header
                    .member()
                    .map(|s| s.as_str().to_string())
                    .unwrap_or_default(),
                path: header
                    .path()
                    .map(|p| p.as_str().to_string())
                    .unwrap_or_default(),
                is_reply: header.reply_serial().is_some(),
                reply_serial: header
                    .reply_serial()
                    .map(|s| s.to_string())
                    .unwrap_or_default(),
                serial: header.primary().serial_num().to_string(),
                message: Some(msg),
                stream_type: t, // Set the stream_type here
                pid: pid_val,
                app_name: app_name_val,
                app_path: app_path_val,
                app_args: app_args_val,
            };

            //push to messages_clone
            messages_clone.lock().await.push(item);
        }
    });

    Ok(messages) // Wrapped messages in Ok()
}
