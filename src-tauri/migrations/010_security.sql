-- Things Shop POS - Endurecimiento de seguridad
-- Version: 010

-- Caducidad de sesiones (epoch segundos). Las sesiones existentes se invalidan.
ALTER TABLE sessions ADD COLUMN expires_at INTEGER NOT NULL DEFAULT 0;
DELETE FROM sessions;

-- Bloqueo de fuerza bruta por usuario.
CREATE TABLE IF NOT EXISTS login_attempts (
    username TEXT PRIMARY KEY,
    failures INTEGER NOT NULL DEFAULT 0,
    locked_until INTEGER NOT NULL DEFAULT 0
);

-- Obliga a cambiar la contraseña por defecto en el primer inicio de sesión.
ALTER TABLE users ADD COLUMN must_change_password INTEGER NOT NULL DEFAULT 0;

-- Índices que faltaban para reportes y purga de bitácora.
CREATE INDEX IF NOT EXISTS idx_app_logs_created ON app_logs(created_at);
CREATE INDEX IF NOT EXISTS idx_sessions_expires ON sessions(expires_at);

INSERT OR IGNORE INTO system_config (key, value, description) VALUES
    ('session_hours', '12', 'Horas de vigencia de la sesión'),
    ('log_retention_days', '90', 'Días de bitácora a conservar'),
    ('update_endpoint', '', 'URL del manifiesto de actualizaciones');

-- Instalaciones existentes siguen con la contraseña por defecto: obligar rotación.
UPDATE users SET must_change_password = 1 WHERE username = 'admin';
