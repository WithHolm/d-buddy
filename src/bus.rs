use anyhow::Result;
use futures::StreamExt;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::fs::read;
use tokio::sync::Mutex;
use tracing::instrument;
use zbus::{fdo::DBusProxy, Connection, MessageStream};

type ProcessInfo = (u32, String, String, Vec<String>);

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
    pub pid: Option<u32>,
    pub app_name: String,
    pub app_path: String,
    pub app_args: Vec<String>,
    pub receiver_pid: Option<u32>,
    pub receiver_app_name: String,
    pub receiver_app_path: String,
    pub receiver_app_args: Vec<String>,
}

impl Default for Item {
    fn default() -> Self {
        Item {
            timestamp: SystemTime::now(),
            sender: String::new(),
            receiver: String::new(),
            member: String::new(),
            path: String::new(),
            message: None,
            serial: String::new(),
            reply_serial: String::new(),
            is_reply: false,
            stream_type: BusType::Session,
            pid: None,
            app_name: String::new(),
            app_path: String::new(),
            app_args: Vec::new(),
            receiver_pid: None,
            receiver_app_name: String::new(),
            receiver_app_path: String::new(),
            receiver_app_args: Vec::new(),
        }
    }
}

impl Item {
    pub fn sender_display(&self) -> std::borrow::Cow<'_, str> {
        if self.app_name != "Unknown" && self.pid.is_some() {
            format!("{}:{}", self.app_name, self.pid.unwrap_or(0)).into()
        } else {
            self.sender.as_str().into()
        }
    }

    pub fn receiver_display(&self) -> std::borrow::Cow<'_, str> {
        if !self.receiver.is_empty() {
            if self.receiver_app_name != "Unknown" && self.receiver_pid.is_some() {
                format!(
                    "{}:{}",
                    self.receiver_app_name,
                    self.receiver_pid.unwrap_or(0)
                )
                .into()
            } else {
                self.receiver.as_str().into()
            }
        } else {
            "".into() // Return an empty Cow::Borrowed("")
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BusType {
    Session = 0,
    System = 1,
    Both = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GroupingType {
    #[default]
    Sender,
    Member,
    Path,
    Serial,
    None,
}

impl std::fmt::Display for GroupingType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GroupingType::Sender => write!(f, "Sender"),
            GroupingType::Member => write!(f, "Member"),
            GroupingType::Path => write!(f, "Path"),
            GroupingType::Serial => write!(f, "Serial"),
            GroupingType::None => write!(f, "None"),
        }
    }
}

impl std::str::FromStr for GroupingType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Sender" => Ok(GroupingType::Sender),
            "Member" => Ok(GroupingType::Member),
            "Path" => Ok(GroupingType::Path),
            "Serial" => Ok(GroupingType::Serial),
            "None" => Ok(GroupingType::None),
            _ => Err(anyhow::anyhow!("Unknown GroupingType: {}", s)),
        }
    }
}

#[instrument(skip(conn, cache))]
async fn get_process_info(
    conn: &zbus::Connection,
    bus_name: &str,
    cache: &Arc<Mutex<HashMap<String, ProcessInfo>>>,
) -> Option<ProcessInfo> {
    {
        let cache_locked = cache.lock().await;
        if let Some(info) = cache_locked.get(bus_name) {
            return Some(info.clone());
        }
    }

    let pid: u32 = conn
        .call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "GetConnectionUnixProcessID",
            &(bus_name),
        )
        .await
        .ok()?
        .body()
        .deserialize()
        .ok()?;

    let cmdline_path = format!("/proc/{}/cmdline", pid);
    let cmdline_content = read(&cmdline_path).await.ok()?;

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

    let info = (pid, app_name, app_path, args);

    {
        let mut cache_locked = cache.lock().await;
        cache_locked.insert(bus_name.to_string(), info.clone());
    }

    Some(info)
}

pub async fn dbus_listener(t: BusType) -> Result<Arc<Mutex<Vec<Item>>>> {
    let messages = Arc::new(Mutex::new(Vec::new()));
    let messages_clone = Arc::clone(&messages);
    let cache = Arc::new(Mutex::new(HashMap::<String, ProcessInfo>::new()));

    let conn: Connection = match t {
        BusType::Session => zbus::Connection::session().await?,
        BusType::System => zbus::Connection::system().await?,
        BusType::Both => zbus::Connection::session().await?,
    };

    if let Some(our_name) = conn.unique_name() {
        // Prime the cache with our own info
        let _ = get_process_info(&conn, our_name.as_str(), &cache).await;
    }

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
    let cache_clone = Arc::clone(&cache);
    tokio::spawn(async move {
        let mut stream = stream;
        while let Some(Ok(msg)) = stream.next().await {
            let header = msg.header();

            let sender_name = header
                .sender()
                .map(|s| s.as_str().to_string())
                .unwrap_or_default();

            let receiver_name = header
                .destination()
                .map(|s| s.as_str().to_string())
                .unwrap_or_default();

            let mut pid_val = None;
            let mut app_name_val = "Unknown".to_string();
            let mut app_path_val = String::new();
            let mut app_args_val = Vec::new();

            if sender_name.starts_with(':') {
                if let Some((p, an, ap, aa)) =
                    get_process_info(&conn, &sender_name, &cache_clone).await
                {
                    pid_val = Some(p);
                    app_name_val = an;
                    app_path_val = ap;
                    app_args_val = aa;
                }
            }

            let mut receiver_pid_val = None;
            let mut receiver_app_name_val = "Unknown".to_string();
            let mut receiver_app_path_val = String::new();
            let mut receiver_app_args_val = Vec::new();

            if receiver_name.starts_with(':') {
                if let Some((p, an, ap, aa)) =
                    get_process_info(&conn, &receiver_name, &cache_clone).await
                {
                    receiver_pid_val = Some(p);
                    receiver_app_name_val = an;
                    receiver_app_path_val = ap;
                    receiver_app_args_val = aa;
                }
            }

            let item = Item {
                timestamp: SystemTime::now(),
                sender: sender_name.clone(),
                receiver: receiver_name.clone(),
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
                stream_type: t,
                pid: pid_val,
                app_name: app_name_val,
                app_path: app_path_val,
                app_args: app_args_val,
                receiver_pid: receiver_pid_val,
                receiver_app_name: receiver_app_name_val,
                receiver_app_path: receiver_app_path_val,
                receiver_app_args: receiver_app_args_val,
            };

            messages_clone.lock().await.push(item);
        }
    });

    Ok(messages)
}
