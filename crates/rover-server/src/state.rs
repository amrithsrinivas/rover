use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// The current schema version. Increment when making schema changes.
const CURRENT_SCHEMA_VERSION: i64 = 1;

/// Persistent state store backed by SQLite.
pub struct StateStore {
    pub(crate) conn: Mutex<Connection>,
}

impl StateStore {
    /// Open or create the SQLite database at the given path.
    pub fn open(path: &Path) -> anyhow::Result<Arc<Self>> {
        let conn = Connection::open(path)?;
        // Enable foreign key enforcement
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        let store = Arc::new(Self {
            conn: Mutex::new(conn),
        });
        store.initialize_schema()?;
        Ok(store)
    }

    // -----------------------------------------------------------------
    // Schema management
    // -----------------------------------------------------------------

    /// Run schema creation and migrations to bring the DB up to date.
    fn initialize_schema(&self) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        let version: i64 = conn
            .query_row(
                "SELECT value FROM server_config WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if version < 1 {
            conn.execute_batch(
                "
                CREATE TABLE IF NOT EXISTS apps (
                    app_id TEXT PRIMARY KEY,
                    name TEXT NOT NULL UNIQUE,
                    runtime TEXT NOT NULL,
                    app_type TEXT NOT NULL,
                    status TEXT NOT NULL,
                    build_command TEXT NOT NULL,
                    run_command TEXT NOT NULL,
                    source_dir TEXT NOT NULL,
                    manifest_toml TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL,
                    restart_count INTEGER DEFAULT 0,
                    pid INTEGER
                );
                CREATE TABLE IF NOT EXISTS env_vars (
                    app_id TEXT NOT NULL,
                    key TEXT NOT NULL,
                    value TEXT NOT NULL,
                    is_secret INTEGER DEFAULT 0,
                    PRIMARY KEY (app_id, key),
                    FOREIGN KEY (app_id) REFERENCES apps(app_id) ON DELETE CASCADE
                );
                CREATE TABLE IF NOT EXISTS auth_tokens (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    token_hash TEXT NOT NULL UNIQUE,
                    created_at INTEGER NOT NULL,
                    last_used_at INTEGER
                );
                CREATE TABLE IF NOT EXISTS server_config (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS logs (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    app_id TEXT NOT NULL,
                    timestamp INTEGER NOT NULL,
                    line TEXT NOT NULL,
                    is_stderr INTEGER DEFAULT 0,
                    FOREIGN KEY (app_id) REFERENCES apps(app_id) ON DELETE CASCADE
                );
                CREATE INDEX IF NOT EXISTS idx_logs_app_time ON logs(app_id, timestamp);
                ",
            )?;

            conn.execute(
                "INSERT OR REPLACE INTO server_config (key, value) VALUES ('schema_version', ?1)",
                params![CURRENT_SCHEMA_VERSION],
            )?;
        }

        // Future migrations go here:
        // if version < 2 { self.migrate_v1_to_v2()?; }
        // if version < 3 { self.migrate_v2_to_v3()?; }

        Ok(())
    }

    // -----------------------------------------------------------------
    // Server config
    // -----------------------------------------------------------------

    /// Read a server config value by key.
    pub fn get_config(&self, key: &str) -> anyhow::Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT value FROM server_config WHERE key = ?1",
            params![key],
            |row| row.get(0),
        );
        match result {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Set a server config value (insert or update).
    pub fn set_config(&self, key: &str, value: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO server_config (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    /// Delete a server config key.
    pub fn delete_config(&self, key: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM server_config WHERE key = ?1", params![key])?;
        Ok(())
    }

    // -----------------------------------------------------------------
    // Apps
    // -----------------------------------------------------------------

    /// Insert a new app. Returns the app_id.
    pub fn insert_app(
        &self,
        app_id: &str,
        name: &str,
        runtime: &str,
        app_type: &str,
        status: &str,
        build_command: &str,
        run_command: &str,
        source_dir: &str,
        manifest_toml: &str,
    ) -> anyhow::Result<()> {
        let now = chrono::Utc::now().timestamp_millis();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO apps (app_id, name, runtime, app_type, status, build_command, run_command, source_dir, manifest_toml, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![app_id, name, runtime, app_type, status, build_command, run_command, source_dir, manifest_toml, now, now],
        )?;
        Ok(())
    }

    /// Retrieve an app by its ID.
    pub fn get_app(&self, app_id: &str) -> anyhow::Result<Option<AppRow>> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT app_id, name, runtime, app_type, status, build_command, run_command, source_dir, manifest_toml, created_at, updated_at, restart_count, pid
             FROM apps WHERE app_id = ?1",
            params![app_id],
            AppRow::from_row,
        );
        match result {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// List apps with optional pagination.
    pub fn list_apps(&self, limit: i64, offset: i64) -> anyhow::Result<Vec<AppRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT app_id, name, runtime, app_type, status, build_command, run_command, source_dir, manifest_toml, created_at, updated_at, restart_count, pid
             FROM apps ORDER BY created_at DESC LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt
            .query_map(params![limit, offset], AppRow::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Update an app's status.
    pub fn update_app_status(&self, app_id: &str, status: &str) -> anyhow::Result<()> {
        let now = chrono::Utc::now().timestamp_millis();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE apps SET status = ?1, updated_at = ?2 WHERE app_id = ?3",
            params![status, now, app_id],
        )?;
        Ok(())
    }

    /// Update an app's PID and increment restart count.
    pub fn update_app_pid(&self, app_id: &str, pid: u32) -> anyhow::Result<()> {
        let now = chrono::Utc::now().timestamp_millis();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE apps SET pid = ?1, restart_count = restart_count + 1, updated_at = ?2 WHERE app_id = ?3",
            params![pid, now, app_id],
        )?;
        Ok(())
    }

    /// Delete an app and all associated data (cascades to env_vars, logs).
    pub fn delete_app(&self, app_id: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM apps WHERE app_id = ?1", params![app_id])?;
        Ok(())
    }

    // -----------------------------------------------------------------
    // Environment variables
    // -----------------------------------------------------------------

    /// Set an environment variable for an app.
    pub fn set_env_var(
        &self,
        app_id: &str,
        key: &str,
        value: &str,
        is_secret: bool,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO env_vars (app_id, key, value, is_secret) VALUES (?1, ?2, ?3, ?4)",
            params![app_id, key, value, is_secret as i32],
        )?;
        Ok(())
    }

    /// Get all environment variables for an app.
    pub fn get_env_vars(&self, app_id: &str) -> anyhow::Result<Vec<EnvVarRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT key, value, is_secret FROM env_vars WHERE app_id = ?1")?;
        let rows = stmt
            .query_map(params![app_id], |row| {
                Ok(EnvVarRow {
                    key: row.get(0)?,
                    value: row.get(1)?,
                    is_secret: row.get::<_, i32>(2)? != 0,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Delete an environment variable.
    pub fn delete_env_var(&self, app_id: &str, key: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM env_vars WHERE app_id = ?1 AND key = ?2",
            params![app_id, key],
        )?;
        Ok(())
    }

    // -----------------------------------------------------------------
    // Logs
    // -----------------------------------------------------------------

    /// Insert a log line for an app.
    pub fn insert_log(
        &self,
        app_id: &str,
        timestamp: i64,
        line: &str,
        is_stderr: bool,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO logs (app_id, timestamp, line, is_stderr) VALUES (?1, ?2, ?3, ?4)",
            params![app_id, timestamp, line, is_stderr as i32],
        )?;
        Ok(())
    }

    /// Get the most recent N log lines for an app.
    pub fn get_logs(&self, app_id: &str, limit: i64) -> anyhow::Result<Vec<LogRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT timestamp, line, is_stderr FROM logs WHERE app_id = ?1 ORDER BY timestamp DESC LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![app_id, limit], |row| {
                Ok(LogRow {
                    timestamp: row.get(0)?,
                    line: row.get(1)?,
                    is_stderr: row.get::<_, i32>(2)? != 0,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        // Return in chronological order
        let mut rows = rows;
        rows.reverse();
        Ok(rows)
    }

    /// Get log lines newer than a given timestamp (for streaming).
    pub fn get_logs_since(
        &self,
        app_id: &str,
        since_timestamp: i64,
    ) -> anyhow::Result<Vec<LogRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT timestamp, line, is_stderr FROM logs WHERE app_id = ?1 AND timestamp > ?2 ORDER BY timestamp ASC",
        )?;
        let rows = stmt
            .query_map(params![app_id, since_timestamp], |row| {
                Ok(LogRow {
                    timestamp: row.get(0)?,
                    line: row.get(1)?,
                    is_stderr: row.get::<_, i32>(2)? != 0,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Delete old log lines, keeping only the most recent `keep_lines`.
    pub fn delete_old_logs(&self, app_id: &str, keep_lines: i64) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM logs WHERE app_id = ?1 AND id NOT IN (
                SELECT id FROM logs WHERE app_id = ?1 ORDER BY timestamp DESC LIMIT ?2
            )",
            params![app_id, keep_lines],
        )?;
        Ok(())
    }

    // -----------------------------------------------------------------
    // Auth tokens
    // -----------------------------------------------------------------

    /// Store an API key hash in the auth_tokens table.
    pub fn insert_auth_token(&self, token_hash: &str) -> anyhow::Result<()> {
        let now = chrono::Utc::now().timestamp_millis();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO auth_tokens (token_hash, created_at) VALUES (?1, ?2)",
            params![token_hash, now],
        )?;
        Ok(())
    }

    /// Check if a token hash exists in the DB, and update last_used_at.
    pub fn verify_auth_token(&self, token_hash: &str) -> anyhow::Result<bool> {
        let now = chrono::Utc::now().timestamp_millis();
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM auth_tokens WHERE token_hash = ?1",
            params![token_hash],
            |row| row.get(0),
        )?;
        if count > 0 {
            conn.execute(
                "UPDATE auth_tokens SET last_used_at = ?1 WHERE token_hash = ?2",
                params![now, token_hash],
            )?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Remove all auth tokens (for resetting pairing).
    pub fn clear_auth_tokens(&self) -> anyhow::Result<()> {
        // Also remove the pairing token from server_config
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM auth_tokens", [])?;
        conn.execute("DELETE FROM server_config WHERE key = 'pairing_token'", [])?;
        Ok(())
    }
}

// -----------------------------------------------------------------
// Row types
// -----------------------------------------------------------------

/// A row from the `apps` table.
#[derive(Debug, Clone)]
pub struct AppRow {
    pub app_id: String,
    pub name: String,
    pub runtime: String,
    pub app_type: String,
    pub status: String,
    pub build_command: String,
    pub run_command: String,
    pub source_dir: String,
    pub manifest_toml: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub restart_count: i64,
    pub pid: Option<i64>,
}

impl AppRow {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            app_id: row.get(0)?,
            name: row.get(1)?,
            runtime: row.get(2)?,
            app_type: row.get(3)?,
            status: row.get(4)?,
            build_command: row.get(5)?,
            run_command: row.get(6)?,
            source_dir: row.get(7)?,
            manifest_toml: row.get(8)?,
            created_at: row.get(9)?,
            updated_at: row.get(10)?,
            restart_count: row.get(11)?,
            pid: row.get(12)?,
        })
    }
}

/// A row from the `env_vars` table.
#[derive(Debug, Clone)]
pub struct EnvVarRow {
    pub key: String,
    pub value: String,
    pub is_secret: bool,
}

/// A row from the `logs` table.
#[derive(Debug, Clone)]
pub struct LogRow {
    pub timestamp: i64,
    pub line: String,
    pub is_stderr: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn open_test_store() -> (Arc<StateStore>, TempDir) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.db");
        let store = StateStore::open(&path).unwrap();
        (store, dir)
    }

    #[test]
    fn test_schema_version_is_set() {
        let (store, _dir) = open_test_store();
        let version = store.get_config("schema_version").unwrap();
        assert_eq!(version, Some("1".to_string()));
    }

    #[test]
    fn test_insert_and_get_app() {
        let (store, _dir) = open_test_store();
        store
            .insert_app(
                "test-id",
                "my-app",
                "python",
                "service",
                "running",
                "pip install",
                "python main.py",
                "/apps/test-id",
                "[app]\nname = 'my-app'",
            )
            .unwrap();

        let app = store.get_app("test-id").unwrap().unwrap();
        assert_eq!(app.name, "my-app");
        assert_eq!(app.status, "running");
        assert_eq!(app.runtime, "python");
    }

    #[test]
    fn test_get_app_not_found() {
        let (store, _dir) = open_test_store();
        let app = store.get_app("nonexistent").unwrap();
        assert!(app.is_none());
    }

    #[test]
    fn test_list_apps() {
        let (store, _dir) = open_test_store();
        store
            .insert_app(
                "a", "app-a", "python", "service", "running", "build", "run", "/a", "toml",
            )
            .unwrap();
        store
            .insert_app(
                "b", "app-b", "node", "job", "stopped", "build", "run", "/b", "toml",
            )
            .unwrap();

        let apps = store.list_apps(50, 0).unwrap();
        assert_eq!(apps.len(), 2);
    }

    #[test]
    fn test_update_status() {
        let (store, _dir) = open_test_store();
        store
            .insert_app("t", "test", "go", "service", "running", "b", "r", "/t", "m")
            .unwrap();
        store.update_app_status("t", "stopped").unwrap();
        let app = store.get_app("t").unwrap().unwrap();
        assert_eq!(app.status, "stopped");
    }

    #[test]
    fn test_delete_app() {
        let (store, _dir) = open_test_store();
        store
            .insert_app(
                "d", "del", "rust", "service", "running", "b", "r", "/d", "m",
            )
            .unwrap();
        store.delete_app("d").unwrap();
        assert!(store.get_app("d").unwrap().is_none());
    }

    #[test]
    fn test_env_vars_roundtrip() {
        let (store, _dir) = open_test_store();
        store
            .insert_app(
                "e", "env-app", "python", "service", "running", "b", "r", "/e", "m",
            )
            .unwrap();
        store.set_env_var("e", "KEY1", "val1", false).unwrap();
        store.set_env_var("e", "SECRET", "shh", true).unwrap();

        let vars = store.get_env_vars("e").unwrap();
        assert_eq!(vars.len(), 2);

        store.delete_env_var("e", "KEY1").unwrap();
        let vars = store.get_env_vars("e").unwrap();
        assert_eq!(vars.len(), 1);
    }

    #[test]
    fn test_logs_insert_and_read() {
        let (store, _dir) = open_test_store();
        store
            .insert_app(
                "l", "log-app", "python", "service", "running", "b", "r", "/l", "m",
            )
            .unwrap();
        store.insert_log("l", 1000, "hello", false).unwrap();
        store.insert_log("l", 2000, "error!", true).unwrap();

        let logs = store.get_logs("l", 10).unwrap();
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].line, "hello");
        assert!(logs[1].is_stderr);
    }

    #[test]
    fn test_log_retention() {
        let (store, _dir) = open_test_store();
        store
            .insert_app(
                "lr",
                "log-retain",
                "python",
                "service",
                "running",
                "b",
                "r",
                "/lr",
                "m",
            )
            .unwrap();
        for i in 0..10 {
            store
                .insert_log("lr", i * 1000, &format!("line {i}"), false)
                .unwrap();
        }
        assert_eq!(store.get_logs("lr", 50).unwrap().len(), 10);

        store.delete_old_logs("lr", 5).unwrap();
        assert_eq!(store.get_logs("lr", 50).unwrap().len(), 5);
    }

    #[test]
    fn test_auth_token_roundtrip() {
        let (store, _dir) = open_test_store();
        store.insert_auth_token("hash123").unwrap();
        assert!(store.verify_auth_token("hash123").unwrap());
        assert!(!store.verify_auth_token("wrong").unwrap());
    }

    #[test]
    fn test_clear_auth_tokens() {
        let (store, _dir) = open_test_store();
        store.insert_auth_token("hash1").unwrap();
        store.set_config("pairing_token", "ptok").unwrap();
        store.clear_auth_tokens().unwrap();
        assert!(!store.verify_auth_token("hash1").unwrap());
        assert!(store.get_config("pairing_token").unwrap().is_none());
    }

    #[test]
    fn test_server_config() {
        let (store, _dir) = open_test_store();
        store.set_config("foo", "bar").unwrap();
        assert_eq!(store.get_config("foo").unwrap(), Some("bar".to_string()));
        store.delete_config("foo").unwrap();
        assert_eq!(store.get_config("foo").unwrap(), None);
    }
}
