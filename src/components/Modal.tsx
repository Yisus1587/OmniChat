import { X } from "lucide-react";
import { useEffect, useRef, type ReactNode } from "react";

/** Diálogo modal nativo (`<dialog>`): foco atrapado y Esc gratis. */
export function Modal({
  title,
  onClose,
  children,
  width = 480,
}: {
  title: string;
  onClose: () => void;
  children: ReactNode;
  width?: number;
}) {
  const ref = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const d = ref.current;
    if (d && !d.open) d.showModal();
  }, []);

  return (
    <dialog
      ref={ref}
      onClose={onClose}
      onCancel={(e) => {
        e.preventDefault();
        onClose();
      }}
      aria-label={title}
      className="m-auto max-h-[88vh] rounded-xl border border-line bg-paper p-0 text-ink shadow-[0_24px_60px_-20px_rgba(23,32,44,0.45)] backdrop:bg-black/50"
      style={{ width }}
    >
      <div className="flex items-center justify-between border-b border-line px-5 py-3.5">
        <h2 className="text-[15px] font-semibold">{title}</h2>
        <button type="button" onClick={onClose} aria-label="Cerrar" className="rounded p-1 text-faint hover:text-ink">
          <X size={16} />
        </button>
      </div>
      <div className="scroll-quiet max-h-[calc(88vh-56px)] overflow-y-auto px-5 py-4">{children}</div>
    </dialog>
  );
}
