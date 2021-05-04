use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::collections::HashMap;

use mediasoup::router::{Router, RouterOptions};
use tokio::sync::{RwLock, broadcast::{self, Sender, Receiver}};

use crate::rtc::get_worker_pool;

#[derive(Clone, Debug)]
pub enum RoomEvent {
    UserJoined(String),
    UserLeft(String),
    RoomDelete,
}

lazy_static! {
    static ref ROOMS: RwLock<HashMap<String, Arc<Room>>> = RwLock::new(HashMap::new());
}

pub struct Room {
    id: String,
    closed: AtomicBool,
    router: Router,
    sender: Sender<RoomEvent>,
}

impl Room {
    pub async fn new(id: String, /* video_allowed: bool */) -> Result<Arc<Self>, ()> {
        let worker = get_worker_pool().get_worker();

        let mut options = RouterOptions::default();
        options.media_codecs.push(crate::rtc::create_opus_codec(2));
        let router = worker.create_router(options).await.map_err(|_| ())?;

        let (sender, _) = broadcast::channel(32);
        info!("Created new room {}", id);
        let room = Arc::new(Room {
            id: id.clone(), closed: AtomicBool::new(false),
            router, sender
        });

        ROOMS.write().await.insert(id, room.clone());

        Ok(room)
    }

    pub async fn get(id: String) -> Option<Arc<Self>> {
        ROOMS.read().await.get(&id).map(|arc| arc.clone())
    }

    pub async fn delete(&self) {
        let result = self.closed.compare_exchange(false, true, Ordering::Release, Ordering::Relaxed);
        if result.is_ok() {
            info!("Deleting room {}", self.id);
            ROOMS.write().await.remove(&self.id);
            self.sender.send(RoomEvent::RoomDelete).ok();
        }
    }

    pub fn closed(&self) -> bool {
        self.closed.load(Ordering::Relaxed)
    }

    pub fn subscribe(&self) -> Option<Receiver<RoomEvent>> {
        match self.closed() {
            false => Some(self.sender.subscribe()),
            true => None,
        }
    }

    pub fn router(&self) -> Option<&Router> {
        match self.closed() {
            false => Some(&self.router),
            true => None,
        }
    }
}