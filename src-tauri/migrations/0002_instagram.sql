-- Añade Instagram (mensajes directos). SQLite no permite modificar un CHECK:
-- se reconstruye la tabla conservando los servicios existentes.

CREATE TABLE services_new (
    id          TEXT PRIMARY KEY NOT NULL,
    kind        TEXT NOT NULL CHECK (kind IN ('whatsapp','telegram','messenger','instagram','gmail','outlook','office365')),
    label       TEXT NOT NULL,
    position    INTEGER NOT NULL,
    notify      INTEGER NOT NULL DEFAULT 1,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

INSERT INTO services_new (id, kind, label, position, notify, created_at, updated_at)
SELECT id, kind, label, position, notify, created_at, updated_at FROM services;

DROP TABLE services;
ALTER TABLE services_new RENAME TO services;
CREATE INDEX idx_services_position ON services(position);
