import {
  createContext,
  useCallback,
  useContext,
  useRef,
  useState,
  useEffect,
  type ReactNode,
} from 'react';
import { createPortal } from 'react-dom';
import { X, CheckCircle2, AlertTriangle, Info, CircleX } from 'lucide-react';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type ToastType = 'success' | 'error' | 'warning' | 'info';

interface ToastItem {
  id: number;
  type: ToastType;
  message: string;
  duration: number;
  exiting: boolean;
}

interface ToastOptions {
  duration?: number;
}

interface ToastContextValue {
  success: (message: string, opts?: ToastOptions) => void;
  error: (message: string, opts?: ToastOptions) => void;
  warning: (message: string, opts?: ToastOptions) => void;
  info: (message: string, opts?: ToastOptions) => void;
}

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

const ToastContext = createContext<ToastContextValue | null>(null);

export function useToast(): ToastContextValue {
  const ctx = useContext(ToastContext);
  if (!ctx) {
    throw new Error('useToast must be used within a <ToastProvider>');
  }
  return ctx;
}

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

const MAX_VISIBLE = 5;
const DEFAULT_DURATION: Record<ToastType, number> = {
  success: 4000,
  info: 4000,
  warning: 6000,
  error: 8000,
};

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<ToastItem[]>([]);
  const idRef = useRef(0);
  const timersRef = useRef<Map<number, ReturnType<typeof setTimeout>>>(new Map());

  const removeToast = useCallback((id: number) => {
    timersRef.current.delete(id);
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const startExit = useCallback(
    (id: number) => {
      setToasts((prev) =>
        prev.map((t) => (t.id === id ? { ...t, exiting: true } : t))
      );
      setTimeout(() => removeToast(id), 200);
    },
    [removeToast]
  );

  const addToast = useCallback(
    (type: ToastType, message: string, opts?: ToastOptions) => {
      const id = ++idRef.current;
      const duration = opts?.duration ?? DEFAULT_DURATION[type];

      setToasts((prev) => {
        const next = [...prev, { id, type, message, duration, exiting: false }];
        if (next.length > MAX_VISIBLE) {
          const oldest = next.find((t) => !t.exiting);
          if (oldest) {
            timersRef.current.delete(oldest.id);
            startExit(oldest.id);
            return next.filter((t) => t.id !== oldest.id).concat([{ id, type, message, duration, exiting: false }]);
          }
        }
        return next;
      });

      const timer = setTimeout(() => startExit(id), duration);
      timersRef.current.set(id, timer);
    },
    [startExit]
  );

  // Cleanup all timers on unmount
  useEffect(() => {
    return () => {
      for (const timer of timersRef.current.values()) {
        clearTimeout(timer);
      }
      timersRef.current.clear();
    };
  }, []);

  const contextValue: ToastContextValue = {
    success: useCallback((msg: string, opts?: ToastOptions) => addToast('success', msg, opts), [addToast]),
    error: useCallback((msg: string, opts?: ToastOptions) => addToast('error', msg, opts), [addToast]),
    warning: useCallback((msg: string, opts?: ToastOptions) => addToast('warning', msg, opts), [addToast]),
    info: useCallback((msg: string, opts?: ToastOptions) => addToast('info', msg, opts), [addToast]),
  };

  return (
    <ToastContext.Provider value={contextValue}>
      {children}
      {createPortal(
        <div className="toast-container" aria-live="polite" aria-label="Notifications">
          {toasts.map((t) => (
            <ToastItemComponent
              key={t.id}
              toast={t}
              onDismiss={() => startExit(t.id)}
            />
          ))}
        </div>,
        document.body
      )}
    </ToastContext.Provider>
  );
}

// ---------------------------------------------------------------------------
// Single Toast
// ---------------------------------------------------------------------------

function ToastIcon({ type }: { type: ToastType }) {
  switch (type) {
    case 'success':
      return <CheckCircle2 size={16} className="toast-icon toast-icon--success" />;
    case 'error':
      return <CircleX size={16} className="toast-icon toast-icon--error" />;
    case 'warning':
      return <AlertTriangle size={16} className="toast-icon toast-icon--warning" />;
    case 'info':
      return <Info size={16} className="toast-icon toast-icon--info" />;
  }
}

function ToastItemComponent({
  toast,
  onDismiss,
}: {
  toast: ToastItem;
  onDismiss: () => void;
}) {
  return (
    <div
      className={[
        'toast',
        `toast--${toast.type}`,
        toast.exiting ? 'toast--exiting' : '',
      ]
        .filter(Boolean)
        .join(' ')}
      role="status"
    >
      <ToastIcon type={toast.type} />
      <span className="toast-message">{toast.message}</span>
      <button
        type="button"
        className="toast-dismiss"
        onClick={onDismiss}
        aria-label="Dismiss notification"
      >
        <X size={13} />
      </button>
    </div>
  );
}
