use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use thiserror::Error;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;
const MAX_USERS: usize = 15;
#[derive(Error, Debug)]
pub enum ServerError {
    #[error("server has already started")]
    Started,
    #[error("database: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("bind: {0}")]
    Bind(#[from] std::io::Error),
}
#[derive(Clone)]
pub struct HostConfig {
    pub room_name: String,
    pub username: String,
    pub password: String,
    pub port: u16,
}
#[derive(Clone)]
struct Room {
    name: String,
    password_hash: String,
}
#[derive(Clone)]
struct Client {
    id: String,
    username: String,
    tx: broadcast::Sender<String>,
    in_voice: bool,
    muted: bool,
}
struct Inner {
    room: Option<Room>,
    clients: HashMap<String, Client>,
    db: Connection,
}
#[derive(Clone)]
pub struct RoomServer {
    inner: Arc<RwLock<Inner>>,
    started: Arc<Mutex<bool>>,
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum Incoming {
    Authenticate {
        username: String,
        password: String,
    },
    #[serde(rename = "message:create")]
    Create {
        content: String,
    },
    #[serde(rename = "message:update")]
    Update {
        id: String,
        content: String,
    },
    #[serde(rename = "message:delete")]
    Delete {
        id: String,
    },
    Typing {
        active: bool,
    },
    #[serde(rename = "voice:state")]
    Voice {
        joined: bool,
        muted: bool,
    },
    Signal {
        to: String,
        data: serde_json::Value,
    },
}
#[derive(Serialize)]
struct ErrorEvent {
    #[serde(rename = "type")]
    kind: &'static str,
    code: &'static str,
    message: String,
}
impl RoomServer {
    pub fn new() -> Self {
        let db = Connection::open_in_memory().expect("open SQLite");
        db.execute_batch("CREATE TABLE rooms (name TEXT NOT NULL, password_hash TEXT NOT NULL); CREATE TABLE messages (id TEXT PRIMARY KEY, author_id TEXT NOT NULL, author_name TEXT NOT NULL, content TEXT NOT NULL, created_at TEXT NOT NULL, edited_at TEXT);").expect("schema");
        Self {
            inner: Arc::new(RwLock::new(Inner {
                room: None,
                clients: HashMap::new(),
                db,
            })),
            started: Arc::new(Mutex::new(false)),
        }
    }
    pub async fn start(&self, config: HostConfig) -> Result<String, ServerError> {
        if *self.started.lock().unwrap() {
            return Err(ServerError::Started);
        };
        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(config.password.as_bytes(), &salt)
            .expect("hash")
            .to_string();
        {
            let mut i = self.inner.write().await;
            i.db.execute(
                "INSERT INTO rooms VALUES (?1,?2)",
                params![config.room_name, hash],
            )?;
            i.room = Some(Room {
                name: config.room_name,
                password_hash: i.db.query_row(
                    "SELECT password_hash FROM rooms LIMIT 1",
                    [],
                    |r| r.get(0),
                )?,
            });
        }
        *self.started.lock().unwrap() = true;
        let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
        let app = Router::new().route("/ws", get(ws)).with_state(self.clone());
        tokio::spawn(async move {
            let listener = tokio::net::TcpListener::bind(addr)
                .await
                .expect("bind server");
            axum::serve(listener, app).await.expect("serve server");
        });
        Ok(format!("ws://127.0.0.1:{}/ws", config.port))
    }
}
async fn ws(upgrade: WebSocketUpgrade, State(server): State<RoomServer>) -> impl IntoResponse {
    upgrade.on_upgrade(move |s| client(s, server))
}
async fn client(mut socket: WebSocket, server: RoomServer) {
    let (tx, mut rx) = broadcast::channel(64);
    let mut id = None::<String>;
    loop {
        tokio::select! { incoming=socket.recv()=> { let Some(Ok(Message::Text(raw)))=incoming else {break}; let Ok(event)=serde_json::from_str::<Incoming>(&raw) else {send(&mut socket,err("invalid_message","Message format is invalid.")).await;continue}; let mut inner=server.inner.write().await; match event { Incoming::Authenticate{username,password}=>{ if id.is_some(){continue} if !valid_name(&username){send(&mut socket,err("invalid_username","Username must be 2–32 printable characters.")).await;continue} let Some(room)=inner.room.clone() else {send(&mut socket,err("offline","Host has closed the room.")).await;continue}; let verified=PasswordHash::new(&room.password_hash).ok().is_some_and(|h|Argon2::default().verify_password(password.as_bytes(),&h).is_ok()); if !verified {send(&mut socket,err("authentication","Incorrect room password.")).await;continue} if inner.clients.len()>=MAX_USERS {send(&mut socket,err("room_full","Room is full. Maximum number of users: 15.")).await;continue} let user_id=Uuid::new_v4().to_string(); inner.clients.insert(user_id.clone(),Client{id:user_id.clone(),username:username.clone(),tx:tx.clone(),in_voice:false,muted:false}); let msgs:Vec<serde_json::Value>=inner.db.prepare("SELECT id,author_id,author_name,content,created_at,edited_at FROM messages ORDER BY created_at DESC LIMIT 100").unwrap().query_map([],|r|Ok(serde_json::json!({"id":r.get::<_,String>(0)?,"authorId":r.get::<_,String>(1)?,"authorName":r.get::<_,String>(2)?,"content":r.get::<_,String>(3)?,"createdAt":r.get::<_,String>(4)?,"editedAt":r.get::<_,Option<String>>(5)?}))).unwrap().filter_map(Result::ok).collect(); let ready=serde_json::json!({"type":"ready","userId":user_id,"room":{"name":room.name,"maxUsers":MAX_USERS},"messages":msgs.into_iter().rev().collect::<Vec<_>>(),"users":users(&inner)}).to_string(); send(&mut socket,ready).await; broadcast(&inner,serde_json::json!({"type":"presence","users":users(&inner)}).to_string()); id=Some(user_id); }, _ if id.is_some()=>{ let uid=id.as_ref().unwrap(); match event { Incoming::Create{content}=>if valid_content(&content){let c=&inner.clients[uid];let msg=serde_json::json!({"id":Uuid::new_v4().to_string(),"authorId":uid,"authorName":c.username,"content":content,"createdAt":chrono_like_now(),"editedAt":null});inner.db.execute("INSERT INTO messages VALUES (?1,?2,?3,?4,?5,NULL)",params![msg["id"].as_str(),uid,c.username,msg["content"].as_str(),msg["createdAt"].as_str()]).ok();broadcast(&inner,serde_json::json!({"type":"message:create","message":msg}).to_string())}, Incoming::Typing{active}=>{let c=&inner.clients[uid];broadcast(&inner,serde_json::json!({"type":"typing","userId":uid,"name":c.username,"active":active}).to_string())}, Incoming::Voice{joined,muted}=>{if let Some(c)=inner.clients.get_mut(uid){c.in_voice=joined;c.muted=muted}broadcast(&inner,serde_json::json!({"type":"presence","users":users(&inner)}).to_string())}, Incoming::Signal{to,data}=>if let Some(c)=inner.clients.get(&to){let _=c.tx.send(serde_json::json!({"type":"signal","from":uid,"data":data}).to_string())}, Incoming::Update{id,content}=>{if valid_content(&content){inner.db.execute("UPDATE messages SET content=?1,edited_at=?2 WHERE id=?3 AND author_id=?4",params![content,chrono_like_now(),id,uid]).ok();}}, Incoming::Delete{id}=>{inner.db.execute("DELETE FROM messages WHERE id=?1 AND author_id=?2",params![id,uid]).ok();broadcast(&inner,serde_json::json!({"type":"message:delete","id":id}).to_string())}, _=>{} } } } }, outgoing=rx.recv()=>if let Ok(text)=outgoing {send(&mut socket,text).await} }
    }
    if let Some(uid) = id {
        let mut i = server.inner.write().await;
        i.clients.remove(&uid);
        broadcast(
            &i,
            serde_json::json!({"type":"presence","users":users(&i)}).to_string(),
        )
    }
}
fn users(i: &Inner) -> Vec<serde_json::Value> {
    i.clients.values().map(|c|serde_json::json!({"id":c.id,"name":c.username,"inVoice":c.in_voice,"muted":c.muted,"speaking":false})).collect()
}
fn broadcast(i: &Inner, v: String) {
    for c in i.clients.values() {
        let _ = c.tx.send(v.clone());
    }
}
async fn send(s: &mut WebSocket, v: String) {
    let _ = s.send(Message::Text(v.into())).await;
}
fn err(code: &'static str, message: &str) -> String {
    serde_json::to_string(&ErrorEvent {
        kind: "error",
        code,
        message: message.into(),
    })
    .unwrap()
}
fn valid_name(s: &str) -> bool {
    s.chars().count() >= 2 && s.chars().count() <= 32 && s.chars().all(|c| !c.is_control())
}
fn valid_content(s: &str) -> bool {
    !s.trim().is_empty() && s.chars().count() <= 4000
}
fn chrono_like_now() -> String {
    format!(
        "{}Z",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    )
}
