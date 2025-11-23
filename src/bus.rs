use anyhow::Result; // Added for error propagation
use futures::StreamExt; // Required for stream.next().await
use std::sync::Arc;
use std::time::SystemTime;
// Removed: use tokio::sync::Mutex;
use zbus::{fdo::DBusProxy, Connection, MessageStream}; // Removed Message from here, as zbus::Message is used in Item struct

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
            stream_type: BusType::Session, // Default value
        }
    }
}

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

impl Default for GroupingType {
    fn default() -> Self {
        GroupingType::Sender
    }
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

// creates a new dbus listener for the given but type. returns a lis
pub async fn dbus_listener(t: BusType) -> Result<Arc<tokio::sync::Mutex<Vec<Item>>>> {
    let messages = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let messages_clone = Arc::clone(&messages);

    //unwrap because if connection fails, the program have no reason to exist..
    let conn: Connection = match t {
        BusType::Session => zbus::Connection::session().await?,
        BusType::System => zbus::Connection::system().await?,
        BusType::Both => {
            // This is a placeholder. "Both" would require merging two streams,
            // which needs a more complex implementation.
            // For now, we can default to session or return an error.
            // Let's default to Session for now to avoid crashing.
            zbus::Connection::session().await?
        }
    };

    let proxy = DBusProxy::new(&conn).await?;
    proxy
        .add_match_rule(
            zbus::MatchRule::builder()
                .msg_type(zbus::message::Type::Signal)
                .build(),
        )
        .await?;

    let stream = MessageStream::from(&conn);
    tokio::spawn(async move {
        let mut stream = stream;
        while let Some(Ok(msg)) = stream.next().await {
            let header = msg.header();
            //create item
            let item = Item {
                timestamp: SystemTime::now(),
                sender: header
                    .sender()
                    .map(|s| s.as_str().to_string())
                    .unwrap_or_default(),
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
            };

            //push to messages_clone
            messages_clone.lock().await.push(item);
        }
    });

    Ok(messages) // Wrapped messages in Ok()
}
