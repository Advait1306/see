use crate::config;
use gpui::*;
use rusqlite::Connection;
use std::sync::Mutex;

pub struct KvStore {
    conn: Mutex<Connection>,
}

impl KvStore {
    pub fn new() -> Self {
        let conn = Self::open_connection();
        Self::init_schema(&conn);
        Self { conn: Mutex::new(conn) }
    }

    fn open_connection() -> Connection {
        let db_path = config::config_dir().join("kv.db");

        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        Connection::open(&db_path).unwrap_or_else(|e| {
            log::warn!("Failed to open KV database at {:?}: {}, using in-memory DB", db_path, e);
            Connection::open_in_memory().expect("Failed to create in-memory database")
        })
    }

    fn init_schema(conn: &Connection) {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS kv (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        )
        .expect("Failed to create KV table");
    }

    pub fn get(&self, key: &str) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT value FROM kv WHERE key = ?1",
            [key],
            |row| row.get(0),
        )
        .ok()
    }

    pub fn set(&self, key: &str, value: &str) {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO kv (key, value) VALUES (?1, ?2)",
            [key, value],
        )
        .expect("Failed to set KV value");
    }

    pub fn delete(&self, key: &str) {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM kv WHERE key = ?1", [key])
            .expect("Failed to delete KV value");
    }
}

pub struct GlobalKvStore(KvStore);

impl Global for GlobalKvStore {}

impl GlobalKvStore {
    pub fn init(cx: &mut App) {
        cx.set_global(GlobalKvStore(KvStore::new()));
    }

    pub fn global(cx: &App) -> &KvStore {
        &cx.global::<GlobalKvStore>().0
    }
}
