//! Memoria que usa cada servicio.
//!
//! Cada servicio tiene su propia carpeta de datos de WebView2, así que su navegador y sus
//! procesos hijos (render, GPU, red…) llevan esa ruta en la línea de comandos. Sumamos la
//! memoria (conjunto de trabajo) de todos los procesos `msedgewebview2.exe` que la llevan.

use std::collections::HashMap;
use std::sync::Mutex;

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

static SYSTEM: Mutex<Option<System>> = Mutex::new(None);

/// Bytes por id de servicio (sólo los servicios con el webview abierto).
pub fn by_service(ids: &[String]) -> HashMap<String, u64> {
    let mut guard = SYSTEM.lock().unwrap();
    if guard.is_none() {
        *guard = System::new().ok();
    }
    let Some(sys) = guard.as_mut() else { return HashMap::new() };
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_memory().with_cmd(UpdateKind::OnlyIfNotSet),
    );

    let mut out: HashMap<String, u64> = HashMap::new();
    for p in sys.processes().values() {
        if !p.name().to_string_lossy().to_ascii_lowercase().starts_with("msedgewebview2") {
            continue;
        }
        let cmd: String = p.cmd().iter().map(|a| a.to_string_lossy()).collect::<Vec<_>>().join(" ");
        if let Some(id) = ids.iter().find(|id| cmd.contains(id.as_str())) {
            *out.entry(id.clone()).or_default() += p.memory();
        }
    }
    out
}
