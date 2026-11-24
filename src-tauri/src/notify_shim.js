// Inyectado en cada webview de servicio ANTES de que cargue la web.
//
// Sustituye la API Notification de la página para que OmniChat muestre una única notificación
// nativa por mensaje (con remitente y texto), en lugar de la de la web + la nuestra.
//
// Canal hacia OmniChat SIN IPC: el aviso se codifica un instante en `document.title` con un
// marcador; Rust lo lee en `on_document_title_changed`, lo filtra y restaura el título.
// La página no obtiene ningún acceso a la app.
(() => {
  if (window.__omnichatShim) return;
  window.__omnichatShim = true;

  const MARK = "⁣omnichat-notify:";
  const queue = [];
  let busy = false;

  function flush() {
    if (busy || queue.length === 0) return;
    busy = true;
    const payload = MARK + queue.shift();
    const previous = document.title;
    document.title = payload;
    setTimeout(() => {
      // Si la web ya cambió el título entretanto, no lo pisamos.
      if (document.title === payload) document.title = previous;
      busy = false;
      flush();
    }, 150);
  }

  function post(title, options) {
    const o = options || {};
    queue.push(
      JSON.stringify({
        t: String(title || "").slice(0, 200),
        b: String(o.body || "").slice(0, 500),
        g: String(o.tag || "").slice(0, 100),
      }),
    );
    if (queue.length > 20) queue.splice(0, queue.length - 20);
    flush();
  }

  class OmniNotification extends EventTarget {
    constructor(title, options) {
      super();
      const o = options || {};
      this.title = String(title || "");
      this.body = String(o.body || "");
      this.tag = String(o.tag || "");
      this.icon = o.icon || "";
      this.data = o.data;
      this.onclick = null;
      this.onshow = null;
      this.onclose = null;
      this.onerror = null;
      post(title, o);
      setTimeout(() => {
        const ev = new Event("show");
        if (typeof this.onshow === "function") this.onshow(ev);
        this.dispatchEvent(ev);
      }, 0);
    }
    close() {}
    static get permission() {
      return "granted";
    }
    static requestPermission(callback) {
      if (typeof callback === "function") callback("granted");
      return Promise.resolve("granted");
    }
    static get maxActions() {
      return 0;
    }
  }

  try {
    Object.defineProperty(window, "Notification", { value: OmniNotification, writable: true, configurable: true });
  } catch (_) {
    window.Notification = OmniNotification;
  }

  // Algunas webs consultan el permiso por la Permissions API.
  if (navigator.permissions && navigator.permissions.query) {
    const query = navigator.permissions.query.bind(navigator.permissions);
    navigator.permissions.query = (desc) =>
      desc && desc.name === "notifications"
        ? Promise.resolve({ state: "granted", name: "notifications", onchange: null, addEventListener() {}, removeEventListener() {} })
        : query(desc);
  }

  // Otras notifican desde su service worker registrado.
  if (window.ServiceWorkerRegistration && ServiceWorkerRegistration.prototype) {
    ServiceWorkerRegistration.prototype.showNotification = function (title, options) {
      post(title, options);
      return Promise.resolve();
    };
  }
})();
