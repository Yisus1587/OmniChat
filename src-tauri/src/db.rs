//! Persistencia: servicios, espacios de trabajo y ajustes.

use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use serde::{de::DeserializeOwned, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;

use crate::catalog::ServiceKind;
use crate::error::{AppError, AppResult};

pub(crate) static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

pub async fn init(db_path: &Path) -> AppResult<SqlitePool> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));
    let options = SqliteConnectOptions::from_str(&url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));
    let pool = SqlitePoolOptions::new().max_connections(4).connect_with(options).await?;
    MIGRATOR.run(&pool).await?;
    Ok(pool)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

// ---------------------------------------------------------------------------
// Servicios
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    pub id: String,
    pub kind: ServiceKind,
    pub label: String,
    pub url: Option<String>,
    pub workspace_id: Option<String>,
    pub position: i64,
    pub notify: bool,
    pub sound: bool,
    pub zoom: f64,
    /// Puede hibernar (cerrarse para liberar memoria) si no se usa durante un rato.
    pub hibernate: bool,
    /// Icono descargado de la web (data URL), sólo en servicios personalizados.
    pub icon: Option<String>,
    /// `None` = el de WebView2; "mobile", "chrome" o un agente personalizado.
    pub user_agent: Option<String>,
    pub proxy: Option<String>,
    pub custom_css: Option<String>,
    pub custom_js: Option<String>,
    /// Carga las extensiones de Chrome de la carpeta de extensiones.
    pub extensions: bool,
}

impl Service {
    /// URL de inicio: la del catálogo o la del usuario (servicios personalizados).
    pub fn home_url(&self) -> Option<&str> {
        self.kind.url().or(self.url.as_deref())
    }
}

const SERVICE_COLUMNS: &str = "id, kind, label, url, workspace_id, position, notify, sound, zoom, hibernate, icon, user_agent, proxy, custom_css, custom_js, extensions";

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<Service>> {
    Ok(sqlx::query_as::<_, Service>(&format!(
        "SELECT {SERVICE_COLUMNS} FROM services ORDER BY position, created_at"
    ))
    .fetch_all(pool)
    .await?)
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<Service> {
    sqlx::query_as::<_, Service>(&format!("SELECT {SERVICE_COLUMNS} FROM services WHERE id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("servicio {id}")))
}

pub async fn create(
    pool: &SqlitePool,
    kind: ServiceKind,
    label: &str,
    url: Option<&str>,
    workspace_id: Option<&str>,
) -> AppResult<Service> {
    create_with_id(pool, &new_id(), kind, label, url, workspace_id).await
}

/// Como `create`, con un id dado (sincronización entre equipos).
pub async fn create_with_id(
    pool: &SqlitePool,
    id: &str,
    kind: ServiceKind,
    label: &str,
    url: Option<&str>,
    workspace_id: Option<&str>,
) -> AppResult<Service> {
    let id = id.to_string();
    let now = now_ms();
    sqlx::query(
        "INSERT INTO services (id, kind, label, url, workspace_id, position, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, (SELECT COALESCE(MAX(position), -1) + 1 FROM services), ?, ?)",
    )
    .bind(&id)
    .bind(kind)
    .bind(label)
    .bind(url)
    .bind(workspace_id)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    get(pool, &id).await
}

/// Cambios parciales de un servicio. `workspace_id = Some("")` lo saca de su espacio.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServicePatch {
    pub label: Option<String>,
    pub notify: Option<bool>,
    pub sound: Option<bool>,
    pub workspace_id: Option<String>,
    pub url: Option<String>,
    pub zoom: Option<f64>,
    pub hibernate: Option<bool>,
    /// Texto opcional: `Some("")` lo borra (vuelve al valor por defecto).
    pub user_agent: Option<String>,
    pub proxy: Option<String>,
    pub custom_css: Option<String>,
    pub custom_js: Option<String>,
    pub extensions: Option<bool>,
}

/// `None` = no tocar; `Some("")` = borrar; `Some(x)` = poner `x`.
fn optional_text(v: &Option<String>) -> (bool, Option<&str>) {
    match v.as_deref() {
        None => (false, None),
        Some("") => (true, None),
        Some(x) => (true, Some(x)),
    }
}

pub async fn update(pool: &SqlitePool, id: &str, p: &ServicePatch) -> AppResult<Service> {
    let (set_ws, ws) = match p.workspace_id.as_deref() {
        None => (false, None),
        Some("") => (true, None),
        Some(w) => (true, Some(w)),
    };
    sqlx::query(
        "UPDATE services SET
            label = COALESCE(?, label),
            notify = COALESCE(?, notify),
            sound = COALESCE(?, sound),
            url = COALESCE(?, url),
            zoom = COALESCE(?, zoom),
            hibernate = COALESCE(?, hibernate),
            extensions = COALESCE(?, extensions),
            user_agent = CASE WHEN ? THEN ? ELSE user_agent END,
            proxy = CASE WHEN ? THEN ? ELSE proxy END,
            custom_css = CASE WHEN ? THEN ? ELSE custom_css END,
            custom_js = CASE WHEN ? THEN ? ELSE custom_js END,
            workspace_id = CASE WHEN ? THEN ? ELSE workspace_id END,
            updated_at = ?
         WHERE id = ?",
    )
    .bind(p.label.as_deref())
    .bind(p.notify)
    .bind(p.sound)
    .bind(p.url.as_deref())
    .bind(p.zoom)
    .bind(p.hibernate)
    .bind(p.extensions)
    .bind(optional_text(&p.user_agent).0)
    .bind(optional_text(&p.user_agent).1)
    .bind(optional_text(&p.proxy).0)
    .bind(optional_text(&p.proxy).1)
    .bind(optional_text(&p.custom_css).0)
    .bind(optional_text(&p.custom_css).1)
    .bind(optional_text(&p.custom_js).0)
    .bind(optional_text(&p.custom_js).1)
    .bind(set_ws)
    .bind(ws)
    .bind(now_ms())
    .bind(id)
    .execute(pool)
    .await?;
    get(pool, id).await
}

pub async fn set_icon(pool: &SqlitePool, id: &str, icon: Option<&str>) -> AppResult<Service> {
    sqlx::query("UPDATE services SET icon = ? WHERE id = ?").bind(icon).bind(id).execute(pool).await?;
    get(pool, id).await
}

pub async fn set_position(pool: &SqlitePool, id: &str, position: i64) -> AppResult<()> {
    sqlx::query("UPDATE services SET position = ? WHERE id = ?").bind(position).bind(id).execute(pool).await?;
    Ok(())
}

pub async fn reorder(pool: &SqlitePool, ids: &[String]) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    for (i, id) in ids.iter().enumerate() {
        sqlx::query("UPDATE services SET position = ? WHERE id = ?")
            .bind(i as i64)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM services WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Espacios de trabajo
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub position: i64,
}

pub async fn list_workspaces(pool: &SqlitePool) -> AppResult<Vec<Workspace>> {
    Ok(sqlx::query_as::<_, Workspace>("SELECT id, name, position FROM workspaces ORDER BY position, created_at")
        .fetch_all(pool)
        .await?)
}

pub async fn create_workspace(pool: &SqlitePool, name: &str) -> AppResult<Workspace> {
    create_workspace_with_id(pool, &new_id(), name).await
}

pub async fn create_workspace_with_id(pool: &SqlitePool, id: &str, name: &str) -> AppResult<Workspace> {
    let id = id.to_string();
    sqlx::query(
        "INSERT INTO workspaces (id, name, position, created_at)
         VALUES (?, ?, (SELECT COALESCE(MAX(position), -1) + 1 FROM workspaces), ?)",
    )
    .bind(&id)
    .bind(name)
    .bind(now_ms())
    .execute(pool)
    .await?;
    Ok(sqlx::query_as::<_, Workspace>("SELECT id, name, position FROM workspaces WHERE id = ?")
        .bind(&id)
        .fetch_one(pool)
        .await?)
}

pub async fn rename_workspace(pool: &SqlitePool, id: &str, name: &str) -> AppResult<()> {
    sqlx::query("UPDATE workspaces SET name = ? WHERE id = ?").bind(name).bind(id).execute(pool).await?;
    Ok(())
}

/// Borra el espacio; sus servicios quedan sin espacio (no se borran).
pub async fn delete_workspace(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE services SET workspace_id = NULL WHERE workspace_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM workspaces WHERE id = ?").bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tareas
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub text: String,
    pub done: bool,
    pub service_id: Option<String>,
    pub created_at: i64,
}

const TASK_COLUMNS: &str = "id, text, done, service_id, created_at";

/// Pendientes primero (las más nuevas arriba), luego las hechas.
pub async fn list_tasks(pool: &SqlitePool) -> AppResult<Vec<Task>> {
    Ok(sqlx::query_as::<_, Task>(&format!("SELECT {TASK_COLUMNS} FROM tasks ORDER BY done, created_at DESC"))
        .fetch_all(pool)
        .await?)
}

pub async fn add_task(pool: &SqlitePool, text: &str, service_id: Option<&str>) -> AppResult<Task> {
    let id = new_id();
    sqlx::query("INSERT INTO tasks (id, text, service_id, created_at) VALUES (?, ?, ?, ?)")
        .bind(&id)
        .bind(text)
        .bind(service_id)
        .bind(now_ms())
        .execute(pool)
        .await?;
    Ok(sqlx::query_as::<_, Task>(&format!("SELECT {TASK_COLUMNS} FROM tasks WHERE id = ?"))
        .bind(&id)
        .fetch_one(pool)
        .await?)
}

pub async fn set_task_done(pool: &SqlitePool, id: &str, done: bool) -> AppResult<()> {
    sqlx::query("UPDATE tasks SET done = ? WHERE id = ?").bind(done).bind(id).execute(pool).await?;
    Ok(())
}

pub async fn delete_task(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM tasks WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

pub async fn clear_done_tasks(pool: &SqlitePool) -> AppResult<()> {
    sqlx::query("DELETE FROM tasks WHERE done = 1").execute(pool).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Ajustes (clave → JSON)
// ---------------------------------------------------------------------------

pub async fn get_setting<T: DeserializeOwned + Default>(pool: &SqlitePool, key: &str) -> AppResult<T> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await?;
    Ok(row.and_then(|(v,)| serde_json::from_str(&v).ok()).unwrap_or_default())
}

pub async fn set_setting<T: Serialize>(pool: &SqlitePool, key: &str, value: &T) -> AppResult<()> {
    let json = serde_json::to_string(value).map_err(|e| AppError::InvalidInput(e.to_string()))?;
    sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
        .bind(key)
        .bind(json)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        MIGRATOR.run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn services_crud_order_and_workspaces() {
        let pool = pool().await;
        let ws = create_workspace(&pool, "Trabajo").await.unwrap();

        let a = create(&pool, ServiceKind::Whatsapp, "WhatsApp", None, None).await.unwrap();
        let b = create(&pool, ServiceKind::Gmail, "Gmail trabajo", None, Some(&ws.id)).await.unwrap();
        let c = create(&pool, ServiceKind::Custom, "Discord", Some("https://discord.com/app"), None).await.unwrap();
        assert_eq!((a.position, b.position, c.position), (0, 1, 2));
        assert!(a.notify && a.sound && a.hibernate);
        assert_eq!(b.workspace_id.as_deref(), Some(ws.id.as_str()));
        assert_eq!(c.home_url(), Some("https://discord.com/app"));
        assert_eq!(a.home_url(), Some("https://web.whatsapp.com/"));

        // Un servicio personalizado sin URL viola el CHECK.
        assert!(create(&pool, ServiceKind::Custom, "X", None, None).await.is_err());

        reorder(&pool, &[b.id.clone(), a.id.clone(), c.id.clone()]).await.unwrap();
        let ids: Vec<_> = list(&pool).await.unwrap().into_iter().map(|s| s.id).collect();
        assert_eq!(ids, vec![b.id.clone(), a.id.clone(), c.id.clone()]);

        let patch = ServicePatch {
            label: Some("Gmail personal".into()),
            notify: Some(false),
            zoom: Some(1.25),
            workspace_id: Some(String::new()),
            hibernate: Some(false),
            ..Default::default()
        };
        let b2 = update(&pool, &b.id, &patch).await.unwrap();
        assert_eq!(b2.label, "Gmail personal");
        assert!(!b2.notify && b2.sound);
        assert_eq!(b2.zoom, 1.25);
        assert_eq!(b2.workspace_id, None);
        assert!(!b2.hibernate);

        // Borrar un espacio deja sus servicios sin espacio.
        update(&pool, &a.id, &ServicePatch { workspace_id: Some(ws.id.clone()), ..Default::default() })
            .await
            .unwrap();
        delete_workspace(&pool, &ws.id).await.unwrap();
        assert_eq!(get(&pool, &a.id).await.unwrap().workspace_id, None);
        assert!(list_workspaces(&pool).await.unwrap().is_empty());

        // Ajustes avanzados: poner y borrar.
        let adv = ServicePatch {
            user_agent: Some("mobile".into()),
            proxy: Some("socks5://127.0.0.1:9050".into()),
            extensions: Some(true),
            ..Default::default()
        };
        let a2 = update(&pool, &a.id, &adv).await.unwrap();
        assert_eq!(a2.user_agent.as_deref(), Some("mobile"));
        assert!(a2.extensions && !c.extensions);
        let a3 = update(&pool, &a.id, &ServicePatch { proxy: Some(String::new()), ..Default::default() }).await.unwrap();
        assert_eq!(a3.proxy, None);
        assert_eq!(a3.user_agent.as_deref(), Some("mobile"));

        // Tareas: ligadas a un servicio, que al borrarse las deja sin servicio.
        let t = add_task(&pool, "Responder a Lucía", Some(&a.id)).await.unwrap();
        add_task(&pool, "Pagar la luz", None).await.unwrap();
        set_task_done(&pool, &t.id, true).await.unwrap();
        delete(&pool, &a.id).await.unwrap();
        let tasks = list_tasks(&pool).await.unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[1].id, t.id);
        assert!(tasks[1].done && tasks[1].service_id.is_none());
        clear_done_tasks(&pool).await.unwrap();
        assert_eq!(list_tasks(&pool).await.unwrap().len(), 1);
        assert_eq!(list(&pool).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn settings_roundtrip() {
        let pool = pool().await;
        let none: Vec<String> = get_setting(&pool, "missing").await.unwrap();
        assert!(none.is_empty());
        set_setting(&pool, "k", &vec!["a".to_string()]).await.unwrap();
        set_setting(&pool, "k", &vec!["b".to_string()]).await.unwrap();
        let v: Vec<String> = get_setting(&pool, "k").await.unwrap();
        assert_eq!(v, vec!["b".to_string()]);
    }
}
