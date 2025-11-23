use anyhow::Result; // Added for error propagation
use futures::StreamExt; // Required for stream.next().await
use std::sync::Arc;
use std::time::SystemTime;
// Removed: use tokio::sync::Mutex;
use zbus::{fdo::DBusProxy, Connection, MessageStream}; // Removed Message from here, as zbus::Message is used in Item struct

#[derive(Debug, Clone)]
pub struct Item {
    timestamp: SystemTime,
    sender: String,
    receiver: String,
    member: String,
    path: String,
    message: Option<zbus::Message>,
    serial: String,
    reply_serial: String,
    is_reply: bool,
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
        }
    }
}

// what type of bus is this?
enum BusType {
    Session,
    System,
}

// creates a new dbus listener for the given but type. returns a lis
async fn dbus_listener(t: BusType) -> Result<Arc<tokio::sync::Mutex<Vec<Item>>>> {
    let messages = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let messages_clone = Arc::clone(&messages);

    //unwrap because if connection fails, the program have no reason to exist..
    let conn: Connection = match t {
        BusType::Session => zbus::Connection::session().await?,
        BusType::System => zbus::Connection::system().await?,
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
                message: msg,
            };

            //push to messages_clone
            messages_clone.lock().await.push(item);
        }
    });

    Ok(messages) // Wrapped messages in Ok()
}
