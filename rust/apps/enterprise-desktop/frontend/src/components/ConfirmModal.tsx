import { useEffect, useRef } from "react";

interface ConfirmModalProps {
  open: boolean;
  title: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  destructive?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmModal({
  open,
  title,
  message,
  confirmLabel = "Confirm",
  cancelLabel = "Cancel",
  destructive = false,
  onConfirm,
  onCancel,
}: ConfirmModalProps) {
  const confirmRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (open) {
      confirmRef.current?.focus();
    }
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onCancel();
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [open, onCancel]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* backdrop */}
      <div
        className="absolute inset-0 bg-black/40 dark:bg-black/60"
        onClick={onCancel}
      />
      {/* dialog */}
      <div className="relative w-full max-w-sm mx-4 rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-primary)] shadow-xl">
        <div className="px-5 pt-5 pb-4">
          <h3 className="text-sm font-semibold text-[var(--color-text-primary)]">
            {title}
          </h3>
          <p className="mt-2 text-sm text-[var(--color-text-secondary)] leading-relaxed">
            {message}
          </p>
        </div>
        <div className="flex justify-end gap-2 px-5 pb-4">
          <button
            type="button"
            onClick={onCancel}
            className="px-3 py-1.5 text-xs font-medium rounded-md border border-[var(--color-border)] text-[var(--color-text-secondary)] hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
          >
            {cancelLabel}
          </button>
          <button
            ref={confirmRef}
            type="button"
            onClick={onConfirm}
            className={`px-3 py-1.5 text-xs font-medium rounded-md transition-colors ${
              destructive
                ? "bg-red-600 text-white hover:bg-red-700 focus:ring-red-500"
                : "bg-brand-600 text-white hover:bg-brand-700 focus:ring-brand-500"
            } focus:outline-none focus:ring-2 focus:ring-offset-1`}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
