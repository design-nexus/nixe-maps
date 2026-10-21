use anyhow::Result;
use nixe_core::paths;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FavoritePlace {
    pub id: i64,
    pub name: String,
    pub label: String, // "favorite", "home", "work"
    pub lat: f64,
    pub lon: f64,
    pub address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecentSearch {
    pub id: i64,
    pub query: String,
    pub formatted_address: String,
    pub lat: f64,
    pub lon: f64,
}

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn open() -> Result<Self> {
        let db_dir = paths::data_home().join("nixe-maps");
        fs::create_dir_all(&db_dir)?;
        let db_path = db_dir.join("maps.db");
        let conn = Connection::open(db_path)?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init()?;
        Ok(db)
    }

    #[cfg(test)]
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init()?;
        Ok(db)
    }

    fn init(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS favorites (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                label TEXT NOT NULL,
                lat REAL NOT NULL,
                lon REAL NOT NULL,
                address TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS recent_searches (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                query TEXT NOT NULL UNIQUE,
                formatted_address TEXT NOT NULL,
                lat REAL NOT NULL,
                lon REAL NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            "#,
        )?;
        Ok(())
    }

    pub fn add_favorite(
        &self,
        name: &str,
        label: &str,
        lat: f64,
        lon: f64,
        address: &str,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        if label == "home" || label == "work" {
            conn.execute("DELETE FROM favorites WHERE label = ?1", params![label])?;
        }
        conn.execute(
            "INSERT INTO favorites (name, label, lat, lon, address) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![name, label, lat, lon, address],
        )?;
        Ok(conn.last_insert_rowid())
    }

    #[allow(dead_code)]
    pub fn remove_favorite(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM favorites WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn list_favorites(&self) -> Result<Vec<FavoritePlace>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, label, lat, lon, address FROM favorites ORDER BY label = 'home' DESC, label = 'work' DESC, id DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(FavoritePlace {
                id: row.get(0)?,
                name: row.get(1)?,
                label: row.get(2)?,
                lat: row.get(3)?,
                lon: row.get(4)?,
                address: row.get(5)?,
            })
        })?;

        let mut places = Vec::new();
        for r in rows {
            places.push(r?);
        }
        Ok(places)
    }

    pub fn get_favorite_by_label(&self, label: &str) -> Result<Option<FavoritePlace>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, label, lat, lon, address FROM favorites WHERE label = ?1 LIMIT 1",
        )?;
        let mut rows = stmt.query_map(params![label], |row| {
            Ok(FavoritePlace {
                id: row.get(0)?,
                name: row.get(1)?,
                label: row.get(2)?,
                lat: row.get(3)?,
                lon: row.get(4)?,
                address: row.get(5)?,
            })
        })?;

        if let Some(r) = rows.next() {
            Ok(Some(r?))
        } else {
            Ok(None)
        }
    }

    pub fn add_recent_search(
        &self,
        query: &str,
        formatted_address: &str,
        lat: f64,
        lon: f64,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO recent_searches (query, formatted_address, lat, lon)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(query) DO UPDATE SET
                formatted_address = excluded.formatted_address,
                lat = excluded.lat,
                lon = excluded.lon,
                created_at = CURRENT_TIMESTAMP
            "#,
            params![query, formatted_address, lat, lon],
        )?;

        conn.execute(
            "DELETE FROM recent_searches WHERE id NOT IN (SELECT id FROM recent_searches ORDER BY created_at DESC LIMIT 20)",
            [],
        )?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn list_recent_searches(&self) -> Result<Vec<RecentSearch>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, query, formatted_address, lat, lon FROM recent_searches ORDER BY created_at DESC, id DESC LIMIT 10",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(RecentSearch {
                id: row.get(0)?,
                query: row.get(1)?,
                formatted_address: row.get(2)?,
                lat: row.get(3)?,
                lon: row.get(4)?,
            })
        })?;

        let mut recents = Vec::new();
        for r in rows {
            recents.push(r?);
        }
        Ok(recents)
    }

    pub fn clear_recent_searches(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM recent_searches", [])?;
        Ok(())
    }
}

pub fn db() -> Result<Database> {
    Database::open()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_favorites_crud() {
        let db = Database::in_memory().unwrap();
        let id = db.add_favorite("Coffee Shop", "favorite", 36.1627, -86.7816, "123 Main St").unwrap();
        assert!(id > 0);

        let favs = db.list_favorites().unwrap();
        assert_eq!(favs.len(), 1);
        assert_eq!(favs[0].name, "Coffee Shop");

        // Add home
        db.add_favorite("My House", "home", 36.1700, -86.7900, "456 Oak St").unwrap();
        let home = db.get_favorite_by_label("home").unwrap();
        assert!(home.is_some());
        assert_eq!(home.unwrap().name, "My House");

        // Remove favorite
        db.remove_favorite(id).unwrap();
        let favs_after = db.list_favorites().unwrap();
        assert_eq!(favs_after.len(), 1); // Only home left
    }

    #[test]
    fn test_recent_searches() {
        let db = Database::in_memory().unwrap();
        db.add_recent_search("Nashville", "Nashville, TN", 36.1627, -86.7816).unwrap();
        db.add_recent_search("Memphis", "Memphis, TN", 35.1495, -90.0490).unwrap();

        let recents = db.list_recent_searches().unwrap();
        assert_eq!(recents.len(), 2);
        assert_eq!(recents[0].query, "Memphis");

        db.clear_recent_searches().unwrap();
        assert!(db.list_recent_searches().unwrap().is_empty());
    }
}
