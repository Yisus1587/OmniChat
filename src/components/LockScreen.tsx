import { Lock } from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { api } from "../lib/api";
import type { AppError, LockStatus } from "../lib/types";

/** Pantalla de bloqueo: tapa toda la ventana (los servicios ya están ocultos desde Rust). */
export function LockScreen({ status, onUnlocked }: { status: LockStatus; onUnlocked: (s: LockStatus) => void }) {
  const [pin, setPin] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [waitUntil, setWaitUntil] = useState(() => (status.retryInMs > 0 ? Date.now() + status.retryInMs : 0));
  const [now, setNow] = useState(() => Date.now());
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (status.retryInMs > 0) setWaitUntil(Date.now() + status.retryInMs);
  }, [status.retryInMs]);

  const waiting = waitUntil > now;
  useEffect(() => {
    if (!waiting) return;
    const t = setInterval(() => setNow(Date.now()), 500);
    return () => clearInterval(t);
  }, [waiting]);

  useEffect(() => {
    if (!waiting && !busy) input.current?.focus();
  }, [waiting, busy]);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (pin.length < 4 || waiting) return;
    setBusy(true);
    setError(null);
    try {
      onUnlocked(await api.unlock(pin));
    } catch (err) {
      setError((err as AppError).message);
      setPin("");
      const s = await api.lockStatus().catch(() => null);
      if (s && s.retryInMs > 0) {
        setWaitUntil(Date.now() + s.retryInMs);
        setNow(Date.now());
      }
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="on-dark grid h-full place-items-center bg-rail text-rail-text">
      <form onSubmit={submit} className="flex w-[280px] flex-col items-center text-center">
        <span className="grid h-14 w-14 place-items-center rounded-2xl bg-rail-2">
          <Lock size={24} />
        </span>
        <h1 className="mt-5 text-[17px] font-semibold">OmniChat está bloqueado</h1>
        <p className="mt-1.5 text-[13px] text-faint">Escribe tu PIN para ver tus servicios.</p>

        <input
          ref={input}
          type="password"
          inputMode="numeric"
          autoComplete="off"
          maxLength={8}
          value={pin}
          disabled={waiting || busy}
          onChange={(e) => setPin(e.target.value.replace(/\D/g, ""))}
          aria-label="PIN"
          className="mt-6 h-12 w-full rounded-lg border border-rail-3 bg-rail-2 text-center text-[22px] tracking-[0.5em] text-rail-text outline-none focus:border-rail-text disabled:opacity-50"
        />
        <p role="alert" className="mt-2 min-h-[20px] text-[12.5px] text-[#f08a8a]">
          {waiting ? `Demasiados intentos. Espera ${Math.ceil((waitUntil - now) / 1000)} s.` : error}
        </p>
        <button
          type="submit"
          disabled={pin.length < 4 || waiting || busy}
          className="btn mt-2 w-full bg-rail-text text-rail hover:bg-white"
        >
          Desbloquear
        </button>
        <p className="mt-6 text-[12px] text-faint">
          Los mensajes siguen llegando. Los avisos no muestran remitente ni texto mientras está bloqueado.
        </p>
      </form>
    </div>
  );
}
