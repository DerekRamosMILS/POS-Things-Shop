use rusqlite::params;
use tauri::State;
use argon2::{self, Argon2, PasswordHasher, PasswordVerifier, password_hash::{SaltString, rand_core::OsRng, PasswordHash}};

use crate::db::connection::DbState;
use crate::models::user::{ChangeOwnPasswordDto, ChangePasswordDto, CreateUserDto, LoginDto, LoginResponse, UpdateUserDto, User};
use crate::session::{now_ts, register_session, require_admin, require_auth, resolve, revoke_session, session_ttl, SessionState};

/// Failed logins tolerated before the account is temporarily locked.
const MAX_LOGIN_FAILURES: i64 = 5;
/// How long an account stays locked after exhausting the attempts.
const LOCKOUT_SECONDS: i64 = 300;
/// Minimum password length enforced everywhere a password is set.
const MIN_PASSWORD_LEN: usize = 8;

/// Same message for unknown user and wrong password so logins cannot be used
/// to enumerate which usernames exist.
const BAD_CREDENTIALS: &str = "Usuario o contraseña incorrectos";

fn hash_password(password: &str) -> Result<String, String> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(format!("La contraseña debe tener al menos {} caracteres", MIN_PASSWORD_LEN));
    }
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| format!("Error al crear hash: {}", e))
}

fn map_user(row: &rusqlite::Row) -> rusqlite::Result<User> {
    Ok(User {
        id: row.get(0)?,
        username: row.get(1)?,
        full_name: row.get(2)?,
        role: row.get(3)?,
        is_active: row.get::<_, i32>(4)? == 1,
        must_change_password: row.get::<_, i32>(5)? == 1,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

const USER_COLUMNS: &str =
    "id, username, full_name, role, is_active, must_change_password, created_at, updated_at";

/// Seconds remaining on an active lockout for this username, if any.
fn lockout_remaining(db: &rusqlite::Connection, username: &str) -> i64 {
    let locked_until: i64 = db
        .query_row(
            "SELECT locked_until FROM login_attempts WHERE username = ?1",
            params![username],
            |row| row.get(0),
        )
        .unwrap_or(0);
    (locked_until - now_ts()).max(0)
}

fn record_failure(db: &rusqlite::Connection, username: &str) {
    db.execute(
        "INSERT INTO login_attempts (username, failures, locked_until) VALUES (?1, 1, 0)
         ON CONFLICT(username) DO UPDATE SET failures = failures + 1",
        params![username],
    )
    .ok();

    let failures: i64 = db
        .query_row(
            "SELECT failures FROM login_attempts WHERE username = ?1",
            params![username],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if failures >= MAX_LOGIN_FAILURES {
        db.execute(
            "UPDATE login_attempts SET failures = 0, locked_until = ?2 WHERE username = ?1",
            params![username, now_ts() + LOCKOUT_SECONDS],
        )
        .ok();
        log::warn!("Cuenta '{}' bloqueada tras {} intentos fallidos", username, failures);
    }
}

fn clear_failures(db: &rusqlite::Connection, username: &str) {
    db.execute("DELETE FROM login_attempts WHERE username = ?1", params![username]).ok();
}

#[tauri::command]
pub fn login(state: State<DbState>, sessions: State<SessionState>, data: LoginDto) -> Result<LoginResponse, String> {
    let db = state.conn();

    let remaining = lockout_remaining(&db, &data.username);
    if remaining > 0 {
        return Err(format!(
            "Demasiados intentos fallidos. Espera {} segundos antes de reintentar.",
            remaining
        ));
    }

    let result = db.query_row(
        "SELECT id, username, full_name, role, is_active, must_change_password, created_at, updated_at, password_hash
         FROM users WHERE username = ?1",
        params![data.username],
        |row| Ok((map_user(row)?, row.get::<_, String>(8)?)),
    );

    let (user, hash) = match result {
        Ok(v) => v,
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            // Still counted so probing for valid usernames gets rate limited too.
            record_failure(&db, &data.username);
            return Err(BAD_CREDENTIALS.to_string());
        }
        Err(e) => return Err(e.to_string()),
    };

    let verified = PasswordHash::new(&hash)
        .ok()
        .map(|parsed| Argon2::default().verify_password(data.password.as_bytes(), &parsed).is_ok())
        .unwrap_or(false);

    if !verified {
        record_failure(&db, &data.username);
        return Err(BAD_CREDENTIALS.to_string());
    }

    // Checked after the password so a wrong password on a disabled account does
    // not reveal that the account exists.
    if !user.is_active {
        return Err("Usuario desactivado".to_string());
    }

    clear_failures(&db, &data.username);

    let token = uuid::Uuid::new_v4().to_string();
    let expires_at = now_ts() + session_ttl(&db);

    db.execute(
        "INSERT INTO app_logs (level, module, message, user_id) VALUES ('info', 'auth', ?1, ?2)",
        params![format!("Inicio de sesión: {}", user.username), user.id],
    ).ok();

    // Persist (single active session per user) + in-memory register.
    db.execute("DELETE FROM sessions WHERE user_id = ?1", params![user.id]).ok();
    db.execute(
        "INSERT INTO sessions (token, user_id, role, expires_at) VALUES (?1, ?2, ?3, ?4)",
        params![token, user.id, user.role, expires_at],
    ).ok();
    register_session(&sessions, &token, user.id, &user.role, expires_at);

    Ok(LoginResponse { user, token })
}

#[tauri::command]
pub fn logout(state: State<DbState>, sessions: State<SessionState>, token: String) -> Result<(), String> {
    {
        let db = state.conn();
        db.execute("DELETE FROM sessions WHERE token = ?1", params![token]).ok();
    }
    revoke_session(&sessions, &token);
    Ok(())
}

/// Confirms a token is still valid; the client calls this on startup so a
/// restored session that has since expired sends the user back to login.
#[tauri::command]
pub fn validate_session(sessions: State<SessionState>, token: String) -> Result<bool, String> {
    Ok(resolve(&sessions, &token).is_ok())
}

#[tauri::command]
pub fn create_user(state: State<DbState>, sessions: State<SessionState>, token: String, data: CreateUserDto) -> Result<User, String> {
    require_admin(&sessions, &token)?;
    let db = state.conn();

    let password_hash = hash_password(&data.password)?;

    db.execute(
        "INSERT INTO users (username, password_hash, full_name, role) VALUES (?1, ?2, ?3, ?4)",
        params![data.username, password_hash, data.full_name, data.role],
    ).map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            "Ya existe un usuario con ese nombre".to_string()
        } else {
            e.to_string()
        }
    })?;

    let id = db.last_insert_rowid();
    db.query_row(
        &format!("SELECT {} FROM users WHERE id = ?1", USER_COLUMNS),
        params![id],
        map_user,
    ).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_users(state: State<DbState>, sessions: State<SessionState>, token: String) -> Result<Vec<User>, String> {
    require_admin(&sessions, &token)?;
    let db = state.conn();

    let mut stmt = db.prepare(
        &format!("SELECT {} FROM users ORDER BY full_name ASC", USER_COLUMNS)
    ).map_err(|e| e.to_string())?;

    let users = stmt
        .query_map([], map_user)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(users)
}

#[tauri::command]
pub fn update_user(state: State<DbState>, sessions: State<SessionState>, token: String, data: UpdateUserDto) -> Result<(), String> {
    require_admin(&sessions, &token)?;
    let db = state.conn();

    let cerrar_sesiones = editar_usuario(&db, &data)?;

    // A deactivated or demoted user must not keep an open session.
    if cerrar_sesiones {
        db.execute("DELETE FROM sessions WHERE user_id = ?1", params![data.id]).ok();
        if let Ok(mut map) = sessions.sessions.lock() {
            map.retain(|_, s| s.user_id != data.id);
        }
    }

    Ok(())
}

/// Núcleo de la edición, con la conexión explícita. Devuelve si hay que cerrarle
/// las sesiones abiertas.
pub fn editar_usuario(db: &rusqlite::Connection, data: &UpdateUserDto) -> Result<bool, String> {
    // Never allow removing the last active administrator.
    let (cur_username, cur_role): (String, String) = db.query_row(
        "SELECT username, role FROM users WHERE id = ?1",
        params![data.id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(|e| e.to_string())?;

    let losing_admin = cur_role == "admin" && (data.role != "admin" || !data.is_active);
    if losing_admin {
        let other_admins: i64 = db.query_row(
            "SELECT COUNT(*) FROM users WHERE role = 'admin' AND is_active = 1 AND id != ?1",
            params![data.id],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        if other_admins == 0 {
            return Err("Debe existir al menos un administrador activo".to_string());
        }
    }

    db.execute_batch("BEGIN TRANSACTION;").map_err(|e| e.to_string())?;
    let resultado = (|| -> Result<(), String> {
        db.execute(
            "UPDATE users SET username=?1, full_name=?2, role=?3, is_active=?4, updated_at=datetime('now','localtime') WHERE id=?5",
            params![data.username, data.full_name, data.role, data.is_active as i32, data.id],
        ).map_err(|e| e.to_string())?;

        if data.username != cur_username {
            mover_bloqueo(db, &cur_username, &data.username)?;
        }
        Ok(())
    })();

    if let Err(e) = resultado {
        db.execute_batch("ROLLBACK;").ok();
        return Err(e);
    }
    crate::db::connection::confirmar(db)?;

    Ok(!data.is_active || data.role != cur_role)
}

/// Le pasa al nombre nuevo el bloqueo por intentos fallidos del nombre viejo.
///
/// El bloqueo se guarda por nombre de usuario, no por id: al renombrar a alguien
/// se quedaba colgado de un nombre que ya nadie usa, así que una cuenta bloqueada
/// se destrababa con solo cambiarle el nombre. Si el nombre nuevo ya traía lo
/// suyo, se queda con lo más estricto de los dos.
fn mover_bloqueo(db: &rusqlite::Connection, viejo: &str, nuevo: &str) -> Result<(), String> {
    db.execute(
        "INSERT INTO login_attempts (username, failures, locked_until)
         SELECT ?2, failures, locked_until FROM login_attempts WHERE username = ?1
         ON CONFLICT(username) DO UPDATE SET
             failures = MAX(failures, excluded.failures),
             locked_until = MAX(locked_until, excluded.locked_until)",
        params![viejo, nuevo],
    ).map_err(|e| e.to_string())?;
    db.execute("DELETE FROM login_attempts WHERE username = ?1", params![viejo])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Admin resets another user's password. The target must change it at next login.
#[tauri::command]
pub fn change_password(state: State<DbState>, sessions: State<SessionState>, token: String, data: ChangePasswordDto) -> Result<(), String> {
    require_admin(&sessions, &token)?;
    let db = state.conn();

    poner_contrasena(&db, data.user_id, &data.new_password)?;

    if let Ok(mut map) = sessions.sessions.lock() {
        map.retain(|_, s| s.user_id != data.user_id);
    }

    Ok(())
}

/// Cambia la contraseña de alguien y le cierra las sesiones, las dos o ninguna.
///
/// Eran dos escrituras sueltas, y el borrado de sesiones iba con `.ok()`: si
/// fallaba, quedaba la contraseña nueva puesta y el renglón de la sesión vieja en
/// la base. Esa sesión revive en el siguiente arranque —`load_sessions` la lee de
/// ahí—, así que quien tenía el token de antes seguía dentro con una contraseña que
/// ya se le cambió. Se restablece una contraseña justamente cuando eso importa.
pub(crate) fn poner_contrasena(
    db: &rusqlite::Connection,
    user_id: i64,
    nueva: &str,
) -> Result<(), String> {
    let password_hash = hash_password(nueva)?;

    db.execute_batch("BEGIN TRANSACTION;").map_err(|e| e.to_string())?;
    let resultado = (|| -> Result<(), String> {
        let filas = db.execute(
            "UPDATE users SET password_hash=?1, must_change_password=1, updated_at=datetime('now','localtime') WHERE id=?2",
            params![password_hash, user_id],
        ).map_err(|e| e.to_string())?;
        if filas == 0 {
            return Err("El usuario no existe".to_string());
        }
        // Force the target back through login with the new password.
        db.execute("DELETE FROM sessions WHERE user_id = ?1", params![user_id])
            .map_err(|e| e.to_string())?;
        Ok(())
    })();

    if let Err(e) = resultado {
        db.execute_batch("ROLLBACK;").ok();
        return Err(e);
    }
    crate::db::connection::confirmar(db)
}

/// Any signed-in user changes their own password, proving the current one first.
#[tauri::command]
pub fn change_own_password(state: State<DbState>, sessions: State<SessionState>, token: String, data: ChangeOwnPasswordDto) -> Result<(), String> {
    let user_id = require_auth(&sessions, &token)?;
    let db = state.conn();

    let current_hash: String = db.query_row(
        "SELECT password_hash FROM users WHERE id = ?1",
        params![user_id],
        |row| row.get(0),
    ).map_err(|e| e.to_string())?;

    let ok = PasswordHash::new(&current_hash)
        .ok()
        .map(|parsed| Argon2::default().verify_password(data.current_password.as_bytes(), &parsed).is_ok())
        .unwrap_or(false);
    if !ok {
        return Err("La contraseña actual es incorrecta".to_string());
    }
    if data.current_password == data.new_password {
        return Err("La nueva contraseña debe ser distinta a la actual".to_string());
    }

    let password_hash = hash_password(&data.new_password)?;
    db.execute(
        "UPDATE users SET password_hash=?1, must_change_password=0, updated_at=datetime('now','localtime') WHERE id=?2",
        params![password_hash, user_id],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

/// Restablece la contraseña de un administrador desde fuera de la interfaz.
///
/// Sin esto, una tienda que olvida la contraseña queda encerrada fuera de sus
/// propios datos: no hay correo de recuperación ni servidor al que pedirle
/// nada, porque todo vive en ese equipo.
///
/// No es un agujero de seguridad: quien puede ejecutar esto ya tiene el archivo
/// de la base en sus manos, y con él puede hacer lo que quiera de todos modos.
/// Queda registrado en la bitácora y obliga a cambiarla al entrar.
pub fn restablecer_admin(
    db: &rusqlite::Connection,
    usuario: &str,
    nueva: &str,
) -> Result<String, String> {
    let (id, rol): (i64, String) = db
        .query_row(
            "SELECT id, role FROM users WHERE username = ?1",
            params![usuario],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| format!("No existe el usuario '{}'", usuario))?;

    if rol != "admin" {
        return Err(format!("'{}' no es administrador", usuario));
    }

    db.execute_batch("BEGIN TRANSACTION;").map_err(|e| e.to_string())?;
    let resultado = (|| -> Result<(), String> {
        let hash = hash_password(nueva)?;
        db.execute(
            "UPDATE users SET password_hash = ?1, is_active = 1, must_change_password = 1,
                    updated_at = datetime('now','localtime')
             WHERE id = ?2",
            params![hash, id],
        )
        .map_err(|e| e.to_string())?;

        // Las sesiones abiertas con la contraseña vieja dejan de servir. Iba con
        // `.ok()`: si fallaba, la contraseña quedaba cambiada y la sesión vieja en
        // la base, lista para revivir en el siguiente arranque. Esto se ejecuta
        // cuando alguien ya no debe poder entrar; es lo último que puede fallar en
        // silencio.
        db.execute("DELETE FROM sessions WHERE user_id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        db.execute("DELETE FROM login_attempts WHERE username = ?1", params![usuario])
            .map_err(|e| e.to_string())?;
        Ok(())
    })();

    if let Err(e) = resultado {
        db.execute_batch("ROLLBACK;").ok();
        return Err(e);
    }
    crate::db::connection::confirmar(db)?;

    db.execute(
        "INSERT INTO app_logs (level, module, message, user_id)
         VALUES ('warn', 'auth', ?1, ?2)",
        params![
            format!("Contraseña de '{}' restablecida desde la línea de comandos", usuario),
            id
        ],
    )
    .ok();

    Ok(format!(
        "Listo. Entra como '{}' con la contraseña que acabas de poner; el sistema te pedirá cambiarla.",
        usuario
    ))
}

/// Ensure at least one admin user exists (called on startup). The seeded
/// account is flagged so the app forces a password change on first login.
pub fn ensure_admin_exists(db: &rusqlite::Connection) -> Result<Option<String>, String> {
    let count: i64 = db.query_row(
        "SELECT COUNT(*) FROM users WHERE role = 'admin'",
        [],
        |row| row.get(0),
    ).map_err(|e| e.to_string())?;

    if count == 0 {
        let clave = clave_inicial();
        let password_hash = hash_password(&clave)?;
        db.execute(
            "INSERT INTO users (username, password_hash, full_name, role, must_change_password)
             VALUES ('admin', ?1, 'Administrador', 'admin', 1)",
            params![password_hash],
        ).map_err(|e| e.to_string())?;

        log::info!("Usuario admin inicial creado; deberá cambiar la contraseña al entrar");
        return Ok(Some(clave));
    }

    Ok(None)
}

/// Contraseña del primer arranque.
///
/// Es fija y conocida a propósito. Se probó generarla al azar, que es lo seguro,
/// y el problema real de esta tienda es el contrario: si nadie la recuerda no
/// hay a quién pedirle un correo de recuperación, porque todo vive en ese
/// equipo. Se decidió que valga más poder entrar siempre.
///
/// Lo que la sostiene: la aplicación obliga a cambiarla en el primer ingreso,
/// así que solo sirve para esa vez, y quien olvide la suya tiene
/// `--restablecer-admin` para volver a entrar.
fn clave_inicial() -> String {
    "admin1234".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn la_clave_inicial_solo_sirve_para_entrar_la_primera_vez() {
        // Es fija y conocida a propósito: la tienda no tiene forma de recuperar
        // una contraseña olvidada. Lo que la sostiene es que se cambia de
        // inmediato, y esto es lo que hay que no romper.
        let conn = db();
        ensure_admin_exists(&conn).unwrap();

        let obligado: i64 = conn.query_row(
            "SELECT must_change_password FROM users WHERE username = 'admin'",
            [], |r| r.get(0)).unwrap();
        assert_eq!(obligado, 1, "debe pedir una contraseña propia al entrar");
    }

    #[test]
    fn el_primer_arranque_entrega_la_clave_y_los_siguientes_no() {
        let conn = db();
        let clave = ensure_admin_exists(&conn).unwrap();
        assert!(clave.is_some(), "el primer arranque debe poder enseñarla");

        assert!(ensure_admin_exists(&conn).unwrap().is_none(),
                "con el admin ya creado no hay clave nueva que enseñar");
    }

    #[test]
    fn passwords_shorter_than_the_minimum_are_rejected() {
        assert!(hash_password("corta1").is_err());
        assert!(hash_password("1234567").is_err());
        assert!(hash_password("12345678").is_ok());
    }

    #[test]
    fn hashes_are_salted_so_equal_passwords_differ() {
        let a = hash_password("misma-clave").unwrap();
        let b = hash_password("misma-clave").unwrap();
        assert_ne!(a, b);
        assert!(a.starts_with("$argon2"));
    }

    #[test]
    fn account_locks_after_max_failures() {
        let conn = db();
        for _ in 0..MAX_LOGIN_FAILURES - 1 {
            record_failure(&conn, "cajero");
        }
        assert_eq!(lockout_remaining(&conn, "cajero"), 0);

        record_failure(&conn, "cajero");
        let left = lockout_remaining(&conn, "cajero");
        assert!(left > 0 && left <= LOCKOUT_SECONDS);
    }

    #[test]
    fn a_successful_login_clears_the_failure_counter() {
        let conn = db();
        record_failure(&conn, "cajero");
        clear_failures(&conn, "cajero");

        let rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM login_attempts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 0);
    }

    #[test]
    fn restablecer_deja_entrar_con_la_nueva_contrasena() {
        let conn = db();
        ensure_admin_exists(&conn).unwrap();

        restablecer_admin(&conn, "admin", "nuevaclave123").unwrap();

        let hash: String = conn.query_row(
            "SELECT password_hash FROM users WHERE username = 'admin'", [], |r| r.get(0)).unwrap();
        let parsed = PasswordHash::new(&hash).unwrap();
        assert!(Argon2::default().verify_password(b"nuevaclave123", &parsed).is_ok());
    }

    #[test]
    fn restablecer_obliga_a_cambiarla_al_entrar() {
        let conn = db();
        ensure_admin_exists(&conn).unwrap();
        conn.execute("UPDATE users SET must_change_password = 0", []).unwrap();

        restablecer_admin(&conn, "admin", "nuevaclave123").unwrap();

        let debe: i64 = conn.query_row(
            "SELECT must_change_password FROM users WHERE username = 'admin'", [], |r| r.get(0)).unwrap();
        assert_eq!(debe, 1);
    }

    #[test]
    fn restablecer_reactiva_una_cuenta_desactivada() {
        // Si el único admin quedó desactivado, restablecer es la salida.
        let conn = db();
        ensure_admin_exists(&conn).unwrap();
        conn.execute("UPDATE users SET is_active = 0", []).unwrap();

        restablecer_admin(&conn, "admin", "nuevaclave123").unwrap();

        let activo: i64 = conn.query_row(
            "SELECT is_active FROM users WHERE username = 'admin'", [], |r| r.get(0)).unwrap();
        assert_eq!(activo, 1);
    }

    #[test]
    fn restablecer_levanta_el_bloqueo_por_intentos_fallidos() {
        let conn = db();
        ensure_admin_exists(&conn).unwrap();
        for _ in 0..MAX_LOGIN_FAILURES {
            record_failure(&conn, "admin");
        }
        assert!(lockout_remaining(&conn, "admin") > 0);

        restablecer_admin(&conn, "admin", "nuevaclave123").unwrap();

        assert_eq!(lockout_remaining(&conn, "admin"), 0);
    }

    #[test]
    fn restablecer_cierra_las_sesiones_abiertas() {
        let conn = db();
        ensure_admin_exists(&conn).unwrap();
        let id: i64 = conn.query_row(
            "SELECT id FROM users WHERE username = 'admin'", [], |r| r.get(0)).unwrap();
        conn.execute(
            "INSERT INTO sessions (token, user_id, role, expires_at) VALUES ('t', ?1, 'admin', 99999999999)",
            params![id],
        ).unwrap();

        restablecer_admin(&conn, "admin", "nuevaclave123").unwrap();

        let quedan: i64 = conn.query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0)).unwrap();
        assert_eq!(quedan, 0);
    }

    #[test]
    fn no_se_restablece_una_contrasena_demasiado_corta() {
        let conn = db();
        ensure_admin_exists(&conn).unwrap();
        assert!(restablecer_admin(&conn, "admin", "corta").is_err());
    }

    #[test]
    fn no_se_restablece_a_alguien_que_no_es_administrador() {
        let conn = db();
        conn.execute(
            "INSERT INTO users (username, password_hash, full_name, role)
             VALUES ('cajero', 'x', 'Cajero', 'cashier')", [],
        ).unwrap();

        let err = restablecer_admin(&conn, "cajero", "nuevaclave123").unwrap_err();
        assert!(err.contains("no es administrador"));
    }

    #[test]
    fn un_usuario_inexistente_da_un_mensaje_claro() {
        let conn = db();
        let err = restablecer_admin(&conn, "nadie", "nuevaclave123").unwrap_err();
        assert!(err.contains("No existe"));
    }

    #[test]
    fn seeded_admin_must_rotate_its_password() {
        let conn = db();
        ensure_admin_exists(&conn).unwrap();

        let must: i64 = conn
            .query_row(
                "SELECT must_change_password FROM users WHERE username = 'admin'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(must, 1);
    }

    #[test]
    fn ensure_admin_is_a_no_op_when_one_exists() {
        let conn = db();
        ensure_admin_exists(&conn).unwrap();
        ensure_admin_exists(&conn).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM users WHERE role = 'admin'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    fn usuario(conn: &rusqlite::Connection, id: i64, nombre: &str, rol: &str) {
        conn.execute(
            "INSERT INTO users (id, username, password_hash, full_name, role) VALUES (?1, ?2, 'x', ?2, ?3)",
            params![id, nombre, rol],
        ).unwrap();
    }

    fn bloqueo(conn: &rusqlite::Connection, nombre: &str) -> Option<(i64, i64)> {
        conn.query_row(
            "SELECT failures, locked_until FROM login_attempts WHERE username = ?1",
            params![nombre],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).ok()
    }

    #[test]
    fn renombrar_a_alguien_no_le_quita_el_bloqueo() {
        // El bloqueo se guarda por nombre, no por id: al renombrar se quedaba
        // colgado de un nombre que ya nadie usa y la cuenta se destrababa sola.
        let conn = db();
        usuario(&conn, 1, "jefa", "admin");
        usuario(&conn, 2, "ana", "cashier");
        let hasta = now_ts() + 300;
        conn.execute(
            "INSERT INTO login_attempts (username, failures, locked_until) VALUES ('ana', 5, ?1)",
            params![hasta],
        ).unwrap();

        editar_usuario(&conn, &UpdateUserDto {
            id: 2, username: "ana.lopez".to_string(), full_name: "Ana López".to_string(),
            role: "cashier".to_string(), is_active: true,
        }).unwrap();

        assert_eq!(bloqueo(&conn, "ana"), None, "no se queda colgado del nombre viejo");
        assert_eq!(bloqueo(&conn, "ana.lopez"), Some((5, hasta)), "el bloqueo sigue a la cuenta");
        assert!(lockout_remaining(&conn, "ana.lopez") > 0);
    }

    #[test]
    fn al_renombrar_se_queda_lo_mas_estricto_de_los_dos_bloqueos() {
        let conn = db();
        usuario(&conn, 1, "jefa", "admin");
        usuario(&conn, 2, "ana", "cashier");
        let lejos = now_ts() + 900;
        conn.execute(
            "INSERT INTO login_attempts (username, failures, locked_until) VALUES ('ana', 2, ?1), ('ana.lopez', 5, ?2)",
            params![now_ts() + 60, lejos],
        ).unwrap();

        editar_usuario(&conn, &UpdateUserDto {
            id: 2, username: "ana.lopez".to_string(), full_name: "Ana".to_string(),
            role: "cashier".to_string(), is_active: true,
        }).unwrap();

        assert_eq!(bloqueo(&conn, "ana.lopez"), Some((5, lejos)));
    }

    #[test]
    fn sin_cambiar_el_nombre_el_bloqueo_se_queda_donde_estaba() {
        let conn = db();
        usuario(&conn, 1, "jefa", "admin");
        usuario(&conn, 2, "ana", "cashier");
        conn.execute(
            "INSERT INTO login_attempts (username, failures, locked_until) VALUES ('ana', 3, 0)", [],
        ).unwrap();

        editar_usuario(&conn, &UpdateUserDto {
            id: 2, username: "ana".to_string(), full_name: "Ana María".to_string(),
            role: "cashier".to_string(), is_active: true,
        }).unwrap();

        assert_eq!(bloqueo(&conn, "ana"), Some((3, 0)));
    }

    #[test]
    fn no_se_puede_quedar_la_tienda_sin_administrador() {
        let conn = db();
        usuario(&conn, 1, "jefa", "admin");

        let e = editar_usuario(&conn, &UpdateUserDto {
            id: 1, username: "jefa".to_string(), full_name: "Jefa".to_string(),
            role: "cashier".to_string(), is_active: true,
        }).unwrap_err();

        assert!(e.contains("administrador"), "mensaje inesperado: {}", e);
        let rol: String = conn.query_row("SELECT role FROM users WHERE id = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(rol, "admin", "no se le cambió el rol");
    }

    fn sesiones_de(conn: &rusqlite::Connection, user_id: i64) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM sessions WHERE user_id = ?1", params![user_id], |r| r.get(0)).unwrap()
    }

    #[test]
    fn cambiar_la_contrasena_no_deja_viva_la_sesion_vieja() {
        // El borrado de sesiones iba con `.ok()`: si fallaba, quedaba la contraseña
        // nueva y el renglón de la sesión vieja en la base, que revive en el
        // siguiente arranque porque `load_sessions` la lee de ahí.
        let conn = db();
        usuario(&conn, 1, "ana", "cashier");
        conn.execute(
            "INSERT INTO sessions (token, user_id, role, expires_at) VALUES ('viejo', 1, 'cashier', ?1)",
            params![now_ts() + 9999],
        ).unwrap();

        poner_contrasena(&conn, 1, "contrasenanueva").unwrap();

        assert_eq!(sesiones_de(&conn, 1), 0, "la sesión vieja no puede quedarse");
        let obliga: i64 = conn
            .query_row("SELECT must_change_password FROM users WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(obliga, 1, "y tiene que cambiarla al entrar");
    }

    #[test]
    fn si_la_contrasena_no_pasa_no_se_toca_nada() {
        // Demasiado corta: ni se cambia el hash ni se cierran sesiones. Antes el
        // orden lo garantizaba por casualidad; ahora lo garantiza la transacción.
        let conn = db();
        usuario(&conn, 1, "ana", "cashier");
        conn.execute(
            "INSERT INTO sessions (token, user_id, role, expires_at) VALUES ('viejo', 1, 'cashier', ?1)",
            params![now_ts() + 9999],
        ).unwrap();
        let antes: String = conn
            .query_row("SELECT password_hash FROM users WHERE id = 1", [], |r| r.get(0))
            .unwrap();

        assert!(poner_contrasena(&conn, 1, "corta").is_err());

        assert_eq!(sesiones_de(&conn, 1), 1, "la sesión sigue porque no hubo cambio");
        let despues: String = conn
            .query_row("SELECT password_hash FROM users WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(antes, despues);
    }

    #[test]
    fn restablecer_a_un_administrador_le_cierra_todo() {
        let conn = db();
        usuario(&conn, 1, "jefa", "admin");
        conn.execute(
            "INSERT INTO sessions (token, user_id, role, expires_at) VALUES ('viejo', 1, 'admin', ?1)",
            params![now_ts() + 9999],
        ).unwrap();
        conn.execute(
            "INSERT INTO login_attempts (username, failures, locked_until) VALUES ('jefa', 5, ?1)",
            params![now_ts() + 600],
        ).unwrap();

        restablecer_admin(&conn, "jefa", "otracontrasena").unwrap();

        assert_eq!(sesiones_de(&conn, 1), 0);
        assert_eq!(bloqueo(&conn, "jefa"), None, "y se destraba para poder entrar");
    }

    #[test]
    fn restablecer_a_quien_no_es_administrador_no_cambia_nada() {
        let conn = db();
        usuario(&conn, 1, "jefa", "admin");
        usuario(&conn, 2, "ana", "cashier");
        let antes: String = conn
            .query_row("SELECT password_hash FROM users WHERE id = 2", [], |r| r.get(0))
            .unwrap();

        assert!(restablecer_admin(&conn, "ana", "otracontrasena").is_err());

        let despues: String = conn
            .query_row("SELECT password_hash FROM users WHERE id = 2", [], |r| r.get(0))
            .unwrap();
        assert_eq!(antes, despues);
    }
}
