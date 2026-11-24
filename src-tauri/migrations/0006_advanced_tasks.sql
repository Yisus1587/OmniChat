-- Ajustes avanzados por servicio.
-- user_agent: NULL = el de WebView2, 'mobile' = vista móvil, 'chrome' = Chrome de escritorio,
-- cualquier otro texto = agente de usuario personalizado.
ALTER TABLE services ADD COLUMN user_agent TEXT;
-- http://host:puerto o socks5://host:puerto
ALTER TABLE services ADD COLUMN proxy TEXT;
ALTER TABLE services ADD COLUMN custom_css TEXT;
ALTER TABLE services ADD COLUMN custom_js TEXT;
-- 1 = carga las extensiones de Chrome de la carpeta de extensiones.
ALTER TABLE services ADD COLUMN extensions INTEGER NOT NULL DEFAULT 0;

-- Lista de tareas (opcionalmente ligada a un servicio).
CREATE TABLE tasks (
    id          TEXT PRIMARY KEY,
    text        TEXT NOT NULL,
    done        INTEGER NOT NULL DEFAULT 0,
    service_id  TEXT REFERENCES services(id) ON DELETE SET NULL,
    created_at  INTEGER NOT NULL
);
