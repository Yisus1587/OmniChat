use serde::{Serialize, Serializer};

pub type AppResult<T> = Result<T, AppError>;

/// Error de la aplicación. Se serializa como `{ kind, message }` para el frontend.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("error de base de datos: {0}")]
    Db(#[from] sqlx::Error),

    #[error("error de migración: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),

    #[error("error de E/S: {0}")]
    Io(#[from] std::io::Error),

    #[error("error de Tauri: {0}")]
    Tauri(#[from] tauri::Error),

    #[error("no encontrado: {0}")]
    NotFound(String),

    #[error("{0}")]
    InvalidInput(String),
}

impl AppError {
    pub fn kind(&self) -> &'static str {
        match self {
            AppError::Db(_) | AppError::Migrate(_) => "database",
            AppError::Io(_) => "io",
            AppError::Tauri(_) => "tauri",
            AppError::NotFound(_) => "not_found",
            AppError::InvalidInput(_) => "invalid_input",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("kind", self.kind())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}
