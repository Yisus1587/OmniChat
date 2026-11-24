-- OmniChat launcher · servicios añadidos por el usuario.
-- Cada fila es una instancia (se puede tener dos WhatsApp o dos Gmail), con su propia
-- sesión aislada en <app_data>/webviews/<id>/.

CREATE TABLE services (
    id          TEXT PRIMARY KEY NOT NULL,
    kind        TEXT NOT NULL CHECK (kind IN ('whatsapp','telegram','messenger','gmail','outlook','office365')),
    label       TEXT NOT NULL,
    position    INTEGER NOT NULL,
    notify      INTEGER NOT NULL DEFAULT 1,   -- notificaciones de escritorio al llegar mensajes
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

CREATE INDEX idx_services_position ON services(position);
