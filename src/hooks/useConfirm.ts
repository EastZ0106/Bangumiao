import { useCallback, useEffect, useRef, useState } from "react";

export interface ConfirmOptions {
  title: string;
  message: string;
  confirmLabel?: string;
  danger?: boolean;
}

interface ConfirmRequest extends ConfirmOptions {
  resolve: (confirmed: boolean) => void;
}

export function useConfirm() {
  const [request, setRequest] = useState<ConfirmRequest | null>(null);
  const requestRef = useRef<ConfirmRequest | null>(null);

  const askConfirm = useCallback((options: ConfirmOptions) => {
    requestRef.current?.resolve(false);
    return new Promise<boolean>((resolve) => {
      const next = { ...options, resolve };
      requestRef.current = next;
      setRequest(next);
    });
  }, []);

  const resolveConfirm = useCallback((confirmed: boolean) => {
    requestRef.current?.resolve(confirmed);
    requestRef.current = null;
    setRequest(null);
  }, []);

  useEffect(() => () => {
    requestRef.current?.resolve(false);
    requestRef.current = null;
  }, []);

  return { request, askConfirm, resolveConfirm };
}
