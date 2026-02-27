use rusqlite::params;
use tauri::State;
use argon2::{self, Argon2, PasswordHasher, PasswordVerifier, password_hash::{SaltString, rand_core::OsRng, PasswordHash}};

use crate::db::connection::DbState;
use crate::models::user::{ChangePasswordDto, CreateUserDto, LoginDto, LoginResponse, UpdateUserDto, User};

#[tauri::command]
pub fn login(state: State<DbState>, data: LoginDto) -> Result<LoginResponse, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let result = db.query_row(
        "SELECT id, username, password_hash, full_name, role, is_active, created_at, updated_at
         FROM users WHERE username = ?1",
        params![data.username],
        |row| {
            Ok((
                User {
                    id: row.get(0)?,
                    username: row.get(1)?,
                    full_name: row.get(3)?,
                    role: row.get(4)?,
                    is_active: row.get::<_, i32>(5)? == 1,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                },
                row.get::<_, String>(2)?,
            ))
        },
    );

    match result {
        Ok((user, hash)) => {
            if !user.is_active {
                return Err("Usuario desactivado".to_string());
            }

            let parsed_hash = PasswordHash::new(&hash)
                .map_err(|_| "Error al verificar contraseña".to_string())?;
            
            Argon2::default()
                .verify_password(data.password.as_bytes(), &parsed_hash)
                .map_err(|_| "Contraseña incorrecta".to_string())?;

            let token = uuid::Uuid::new_v4().to_string();

            Ok(LoginResponse { user, token })
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            Err("Usuario no encontrado".to_string())
        }
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub fn create_user(state: State<DbState>, data: CreateUserDto) -> Result<User, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let salt = SaltString::generate(&mut OsRng);
    let password_hash = Argon2::default()
        .hash_password(data.password.as_bytes(), &salt)
        .map_err(|e| format!("Error al crear hash: {}", e))?
        .to_string();

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
        "SELECT id, username, full_name, role, is_active, created_at, updated_at FROM users WHERE id = ?1",
        params![id],
        |row| {
            Ok(User {
                id: row.get(0)?,
                username: row.get(1)?,
                full_name: row.get(2)?,
                role: row.get(3)?,
                is_active: row.get::<_, i32>(4)? == 1,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        },
    ).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_users(state: State<DbState>) -> Result<Vec<User>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let mut stmt = db.prepare(
        "SELECT id, username, full_name, role, is_active, created_at, updated_at FROM users ORDER BY full_name ASC"
    ).map_err(|e| e.to_string())?;

    let users = stmt
        .query_map([], |row| {
            Ok(User {
                id: row.get(0)?,
                username: row.get(1)?,
                full_name: row.get(2)?,
                role: row.get(3)?,
                is_active: row.get::<_, i32>(4)? == 1,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(users)
}

#[tauri::command]
pub fn update_user(state: State<DbState>, data: UpdateUserDto) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    db.execute(
        "UPDATE users SET username=?1, full_name=?2, role=?3, is_active=?4, updated_at=datetime('now','localtime') WHERE id=?5",
        params![data.username, data.full_name, data.role, data.is_active as i32, data.id],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn change_password(state: State<DbState>, data: ChangePasswordDto) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let salt = SaltString::generate(&mut OsRng);
    let password_hash = Argon2::default()
        .hash_password(data.new_password.as_bytes(), &salt)
        .map_err(|e| format!("Error al crear hash: {}", e))?
        .to_string();

    db.execute(
        "UPDATE users SET password_hash=?1, updated_at=datetime('now','localtime') WHERE id=?2",
        params![password_hash, data.user_id],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

/// Ensure at least one admin user exists (called on startup)
pub fn ensure_admin_exists(db: &rusqlite::Connection) -> Result<(), String> {
    let count: i64 = db.query_row(
        "SELECT COUNT(*) FROM users WHERE role = 'admin'",
        [],
        |row| row.get(0),
    ).map_err(|e| e.to_string())?;

    if count == 0 {
        let salt = SaltString::generate(&mut OsRng);
        let password_hash = Argon2::default()
            .hash_password(b"admin123", &salt)
            .map_err(|e| format!("Error al crear hash: {}", e))?
            .to_string();

        db.execute(
            "INSERT INTO users (username, password_hash, full_name, role) VALUES ('admin', ?1, 'Administrador', 'admin')",
            params![password_hash],
        ).map_err(|e| e.to_string())?;

        log::info!("Default admin user created (username: admin, password: admin123)");
    }

    Ok(())
}
