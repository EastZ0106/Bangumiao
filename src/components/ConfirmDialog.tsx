import { useEffect, useRef } from "react";
import type { ConfirmOptions } from "../hooks/useConfirm";

interface ConfirmDialogProps {
  request: ConfirmOptions | null;
  onResolve: (confirmed: boolean) => void;
}

export default function ConfirmDialog({ request, onResolve }: ConfirmDialogProps) {
  const cancelRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!request) return;
    const previousFocus = document.activeElement as HTMLElement | null;
    cancelRef.current?.focus();
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onResolve(false);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      previousFocus?.focus();
    };
  }, [request, onResolve]);

  if (!request) return null;

  return (
    <div className="modal-backdrop" onMouseDown={() => onResolve(false)}>
      <div
        className="confirm-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="confirm-dialog-title"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <h2 id="confirm-dialog-title">{request.title}</h2>
        <p>{request.message}</p>
        <div className="confirm-dialog-actions">
          <button ref={cancelRef} className="btn btn-ghost" onClick={() => onResolve(false)}>
            取消
          </button>
          <button
            className={request.danger ? "btn btn-danger" : "btn btn-primary"}
            onClick={() => onResolve(true)}
          >
            {request.confirmLabel ?? "确认"}
          </button>
        </div>
      </div>
    </div>
  );
}
