import type { ToastState } from "../hooks/useToast";

interface ToastProps {
  toast: ToastState;
}

export default function Toast({ toast }: ToastProps) {
  if (!toast) return null;

  return (
    <div
      className={`toast ${toast.ok ? "toast-success" : "toast-error"}`}
      role="status"
      aria-live="polite"
    >
      {toast.ok ? "✓ " : "✗ "}
      {toast.text}
    </div>
  );
}
