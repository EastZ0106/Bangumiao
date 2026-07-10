import { useCallback, useEffect, useRef, useState } from "react";

export interface ToastMessage {
  text: string;
  ok: boolean;
}

export type ToastState = ToastMessage | null;

export function useToast(durationMs = 4000) {
  const [toast, setToast] = useState<ToastState>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const clearToast = useCallback(() => {
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    setToast(null);
  }, []);

  const showToast = useCallback((text: string, ok: boolean) => {
    if (timerRef.current) {
      clearTimeout(timerRef.current);
    }
    setToast({ text, ok });
    timerRef.current = setTimeout(() => {
      setToast(null);
      timerRef.current = null;
    }, durationMs);
  }, [durationMs]);

  useEffect(() => {
    return () => {
      if (timerRef.current) {
        clearTimeout(timerRef.current);
      }
    };
  }, []);

  return { toast, showToast, clearToast };
}
