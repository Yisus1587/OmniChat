# OmniChat

OmniChat reúne servicios de mensajería y correo web en una aplicación de escritorio. Cada servicio se abre en su propio webview y conserva una sesión independiente.

Inicia sesión en las páginas oficiales de los servicios; OmniChat no requiere una cuenta propia ni credenciales de API.

## Servicios

| Servicio | Sitio |
|---|---|
| WhatsApp | [web.whatsapp.com](https://web.whatsapp.com/) |
| Telegram | [web.telegram.org](https://web.telegram.org/a/) |
| Messenger | [facebook.com/messages](https://www.facebook.com/messages/) |
| Instagram | [instagram.com/direct/inbox](https://www.instagram.com/direct/inbox/) |
| Gmail | [mail.google.com](https://mail.google.com/) |
| Outlook personal | [outlook.live.com](https://outlook.live.com/mail/) |
| Microsoft 365 | [outlook.office.com](https://outlook.office.com/mail/) |

También puedes añadir sitios personalizados que usen HTTP o HTTPS, como Discord, Slack o LinkedIn. Es posible añadir varias cuentas del mismo servicio; cada una mantiene su propia sesión.

## Funciones

- **Sesiones independientes:** cada servicio usa su propio perfil de WebView2.
- **Notificaciones y no leídos:** OmniChat muestra notificaciones de escritorio y lee los contadores de no leídos del título de cada sitio cuando están disponibles.
- **Bandeja del sistema:** oculta la ventana sin cerrar los servicios; incluye controles para salir y activar «No molestar».
- **Espacios de trabajo:** organiza los servicios, por ejemplo, en «Personal» y «Trabajo».
- **Concentración:** silencia notificaciones por un periodo o durante un horario diario.
- **Atajos de teclado:** cambia de servicio, busca y ajusta el zoom.
- **Centro de notificaciones y tareas:** conserva avisos durante la sesión y permite crear tareas locales vinculadas a un servicio.
- **Ahorro de memoria:** hiberna servicios inactivos; no recibirán notificaciones mientras estén hibernados.
- **Bloqueo con PIN:** bloquea la interfaz y oculta el contenido de los avisos. El PIN no cifra los datos guardados por los sitios.
- **Copia de seguridad e importación:** exporta servicios, espacios de trabajo y ajustes a JSON. Las sesiones y el PIN no se incluyen.
- **Sincronización entre equipos:** comparte configuración mediante un archivo en una carpeta que ya sincronices con OneDrive, Dropbox, Google Drive u otro servicio compatible. No se sincronizan sesiones, PIN, proxy, CSS, JavaScript ni extensiones.
- **Ajustes por servicio:** tema, sonido, avisos, zoom, agente de usuario, proxy, CSS o JavaScript propios y extensiones desempaquetadas compatibles con WebView2.

## Privacidad y seguridad

- Los sitios se cargan directamente desde sus proveedores y gestionan sus propios inicios de sesión. Las sesiones se guardan localmente en perfiles separados de WebView2.
- OmniChat guarda la lista de servicios, espacios y preferencias en una base de datos SQLite local. La sincronización opcional escribe configuración en la carpeta que elijas; OmniChat no opera un servidor de sincronización.
- Los webviews de los servicios no reciben permisos IPC de Tauri; esos permisos están limitados a la interfaz local de OmniChat.
- El PIN es una protección para ocultar la aplicación a quien use el equipo brevemente. No cifra las cookies ni los datos de los perfiles web.
- El CSS, JavaScript y las extensiones que instales se ejecutan en el sitio al que los asignes. Usa únicamente código y extensiones de fuentes confiables.

## Requisitos

La plataforma objetivo es **Windows 11 con WebView2 Runtime**. El soporte para otros sistemas operativos no está documentado.

Para desarrollar o generar instaladores necesitas:

- Rust estable con la herramienta MSVC.
- Node.js **20.19 o posterior** o **22.12 o posterior**.
- Visual Studio Build Tools 2022 con «Desarrollo para el escritorio con C++» y Windows 11 SDK.
- WebView2 Runtime.

## Desarrollo

Desde la carpeta del proyecto, en PowerShell:

```powershell
npm ci
npm run tauri dev
```

Para abrir únicamente la interfaz en el navegador, usa `npm run dev`. En ese modo se emplean datos de demostración y no se cargan los sitios como servicios de escritorio.

Comandos adicionales:

```powershell
npm run typecheck
npm run build
npm run tauri build
```

`npm run tauri build` genera los paquetes de Windows en `src-tauri/target/release/bundle/`.

Para ejecutar las pruebas Rust:

```powershell
cd src-tauri
cargo test
```

## Estructura del proyecto

```text
src/                  Interfaz React y componentes
src/lib/               API de la app, catálogo, tipos y datos de demostración
src-tauri/src/         Aplicación Tauri en Rust
src-tauri/migrations/  Migraciones de SQLite
src-tauri/capabilities/ Permisos de la interfaz local
```

## Limitaciones

- El contador de no leídos depende de que cada sitio publique esa información en el título de su página.
- Los sitios pueden cambiar su interfaz, limitar el acceso desde un webview o solicitar verificaciones adicionales al iniciar sesión.
- Los avisos y las sesiones dependen de los sitios y del runtime WebView2. OmniChat no puede garantizar que cada servicio mantenga el mismo comportamiento con el tiempo.

## Créditos

Los logotipos de los servicios proceden de [gilbarbara/logos](https://github.com/gilbarbara/logos) (CC0) y [vscode-icons](https://github.com/vscode-icons/vscode-icons) (MIT, icono de Outlook), vía Iconify. Las marcas pertenecen a sus respectivos titulares y se usan para identificar los servicios.

El proyecto declara licencia MIT en `src-tauri/Cargo.toml`.
