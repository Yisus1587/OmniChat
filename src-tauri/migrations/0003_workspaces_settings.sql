-- Espacios de trabajo, servicios personalizados (cualquier URL), zoom y sonido por servicio,
-- y ajustes generales de la app.

CREATE TABLE workspaces (
    id          TEXT PRIMARY KEY NOT NULL,
    name        TEXT NOT NULL,
    position    INTEGER NOT NULL,
    created_at  INTEGER NOT NULL
);

-- SQLite no permite modificar un CHECK: se reconstruye `services` conservando los datos.
CREATE TABLE services_new (
    id            TEXT PRIMARY KEY NOT NULL,
    kind          TEXT NOT NULL CHECK (kind IN ('whatsapp','telegram','messenger','instagram','gmail','outlook','office365','custom')),
    label         TEXT NOT NULL,
    url           TEXT,                        -- sólo para `custom`
    workspace_id  TEXT REFERENCES workspaces(id) ON DELETE SET NULL,
    position      INTEGER NOT NULL,
    notify        INTEGER NOT NULL DEFAULT 1,
    sound         INTEGER NOT NULL DEFAULT 1,
    zoom          REAL NOT NULL DEFAULT 1.0,
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL,
    CHECK (kind <> 'custom' OR url IS NOT NULL)
);

INSERT INTO services_new (id, kind, label, position, notify, created_at, updated_at)
SELECT id, kind, label, position, notify, created_at, updated_at FROM services;

DROP TABLE services;
ALTER TABLE services_new RENAME TO services;
CREATE INDEX idx_services_position ON services(position);
CREATE INDEX idx_services_workspace ON services(workspace_id);

-- Ajustes clave → valor JSON.
CREATE TABLE settings (
    key    TEXT PRIMARY KEY NOT NULL,
    value  TEXT NOT NULL
);
