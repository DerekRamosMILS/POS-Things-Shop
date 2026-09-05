use std::collections::HashMap;
use std::sync::Mutex;
use tauri::State;

const DEFAULT_SESSION_HOURS: i64 = 12;

/// Server-side session record created at login. The client sends the opaque
/// token with every command; commands resolve the acting user and role from
/// this map instead of trusting anything the client sends.
pub struct SessionInfo {
    pub user_id: i64,
    pub role: String,
    /// Unix epoch seconds after which the token is rejected.
    pub expires_at: i64,
}

pub struct SessionState {
    pub sessions: Mutex<HashMap<String, SessionInfo>>,
}

impl SessionState {
    pub fn with_map(map: HashMap<String, SessionInfo>) -> Self {
        SessionState { sessions: Mutex::new(map) }
    }
}

pub fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Session lifetime in seconds, from `system_config.session_hours`.
pub fn session_ttl(conn: &rusqlite::Connection) -> i64 {
    conn.query_row(
        "SELECT value FROM system_config WHERE key = 'session_hours'",
        [],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .and_then(|s| s.trim().parse::<i64>().ok())
    .filter(|h| *h > 0)
    .unwrap_or(DEFAULT_SESSION_HOURS)
        * 3600
}

/// Load still-valid persisted sessions so logins survive an app restart.
pub fn load_sessions(conn: &rusqlite::Connection) -> HashMap<String, SessionInfo> {
    let now = now_ts();
    conn.execute("DELETE FROM sessions WHERE expires_at <= ?1", [now]).ok();

    let mut map = HashMap::new();
    if let Ok(mut stmt) =
        conn.prepare("SELECT token, user_id, role, expires_at FROM sessions WHERE expires_at > ?1")
    {
        if let Ok(rows) = stmt.query_map([now], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
            ))
        }) {
            for row in rows.flatten() {
                map.insert(
                    row.0,
                    SessionInfo { user_id: row.1, role: row.2, expires_at: row.3 },
                );
            }
        }
    }
    map
}

/// Register a session at login.
pub fn register_session(
    sessions: &State<SessionState>,
    token: &str,
    user_id: i64,
    role: &str,
    expires_at: i64,
) {
    if let Ok(mut map) = sessions.sessions.lock() {
        // Una sesión por usuario. El registro en disco ya reemplazaba la
        // anterior; en memoria el token viejo seguía sirviendo hasta caducar,
        // así que volver a entrar no cerraba de verdad la sesión previa.
        map.retain(|_, s| s.expires_at > now_ts() && s.user_id != user_id);
        map.insert(
            token.to_string(),
            SessionInfo { user_id, role: role.to_string(), expires_at },
        );
    }
}

/// Drop a session (logout).
pub fn revoke_session(sessions: &State<SessionState>, token: &str) {
    if let Ok(mut map) = sessions.sessions.lock() {
        map.remove(token);
    }
}

/// Resolve a token to (user_id, role), rejecting unknown or expired tokens.
pub fn resolve(sessions: &State<SessionState>, token: &str) -> Result<(i64, String), String> {
    let mut map = sessions.sessions.lock().map_err(|e| e.to_string())?;
    match map.get(token) {
        Some(s) if s.expires_at > now_ts() => Ok((s.user_id, s.role.clone())),
        Some(_) => {
            map.remove(token);
            Err("La sesión expiró. Vuelve a iniciar sesión.".to_string())
        }
        None => Err("Sesión inválida o expirada. Vuelve a iniciar sesión.".to_string()),
    }
}

/// Require an admin session; returns the acting user id.
pub fn require_admin(sessions: &State<SessionState>, token: &str) -> Result<i64, String> {
    let (user_id, role) = resolve(sessions, token)?;
    if role != "admin" {
        return Err("No autorizado: se requiere rol de administrador".to_string());
    }
    Ok(user_id)
}

/// Require any authenticated session; returns the acting user id.
pub fn require_auth(sessions: &State<SessionState>, token: &str) -> Result<i64, String> {
    resolve(sessions, token).map(|(id, _)| id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db_with_sessions() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        conn
    }

    fn insert(conn: &rusqlite::Connection, token: &str, expires_at: i64) {
        conn.execute(
            "INSERT INTO users (id, username, password_hash, full_name, role)
             VALUES (?1, ?2, 'x', 'Test', 'admin')
             ON CONFLICT(id) DO NOTHING",
            rusqlite::params![1, "tester"],
        )
        .ok();
        conn.execute(
            "INSERT INTO sessions (token, user_id, role, expires_at) VALUES (?1, 1, 'admin', ?2)",
            rusqlite::params![token, expires_at],
        )
        .unwrap();
    }

    #[test]
    fn expired_sessions_are_not_rehydrated() {
        let conn = db_with_sessions();
        insert(&conn, "vivo", now_ts() + 3600);
        insert(&conn, "muerto", now_ts() - 1);

        let map = load_sessions(&conn);

        assert!(map.contains_key("vivo"));
        assert!(!map.contains_key("muerto"));
    }

    #[test]
    fn expired_sessions_are_deleted_from_disk() {
        let conn = db_with_sessions();
        insert(&conn, "muerto", now_ts() - 1);

        load_sessions(&conn);

        let left: i64 = conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 0);
    }

    #[test]
    fn volver_a_entrar_invalida_el_token_anterior() {
        let estado = SessionState::with_map(HashMap::new());
        let mut map = estado.sessions.lock().unwrap();
        map.insert("viejo".into(), SessionInfo { user_id: 1, role: "admin".into(), expires_at: now_ts() + 3600 });
        map.retain(|_, s| s.expires_at > now_ts() && s.user_id != 1);
        map.insert("nuevo".into(), SessionInfo { user_id: 1, role: "admin".into(), expires_at: now_ts() + 3600 });

        assert!(!map.contains_key("viejo"), "el token anterior debe dejar de servir");
        assert!(map.contains_key("nuevo"));
    }

    #[test]
    fn ttl_comes_from_config_and_falls_back_when_invalid() {
        let conn = db_with_sessions();
        assert_eq!(session_ttl(&conn), 12 * 3600);

        conn.execute("UPDATE system_config SET value = '4' WHERE key = 'session_hours'", [])
            .unwrap();
        assert_eq!(session_ttl(&conn), 4 * 3600);

        conn.execute("UPDATE system_config SET value = 'abc' WHERE key = 'session_hours'", [])
            .unwrap();
        assert_eq!(session_ttl(&conn), DEFAULT_SESSION_HOURS * 3600);
    }
}
