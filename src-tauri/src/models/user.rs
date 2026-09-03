use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub full_name: String,
    pub role: String,
    pub is_active: bool,
    pub must_change_password: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserDto {
    pub username: String,
    pub password: String,
    pub full_name: String,
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserDto {
    pub id: i64,
    pub username: String,
    pub full_name: String,
    pub role: String,
    pub is_active: bool,
}

#[derive(Debug, Deserialize)]
pub struct LoginDto {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub user: User,
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordDto {
    pub user_id: i64,
    pub new_password: String,
}

#[derive(Debug, Deserialize)]
pub struct ChangeOwnPasswordDto {
    pub current_password: String,
    pub new_password: String,
}
