-- Hibernación: 1 = el servicio puede cerrarse (liberando memoria) si no se usa un rato.
ALTER TABLE services ADD COLUMN hibernate INTEGER NOT NULL DEFAULT 1;
