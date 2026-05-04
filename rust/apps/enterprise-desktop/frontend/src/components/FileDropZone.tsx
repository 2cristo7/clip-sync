import { useCallback, useRef, useState } from "react";

const MAX_FILE_SIZE = 50 * 1024 * 1024; // 50 MB

export interface SelectedFile {
  file: File;
  name: string;
  size: number;
  type: string;
}

interface FileDropZoneProps {
  onFilesSelected: (files: SelectedFile[]) => void;
  disabled?: boolean;
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function FileDropZone({ onFilesSelected, disabled }: FileDropZoneProps) {
  const [dragging, setDragging] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const dragCounter = useRef(0);

  const processFiles = useCallback(
    (fileList: FileList | null) => {
      if (!fileList || fileList.length === 0) return;

      setError(null);
      const accepted: SelectedFile[] = [];
      const rejected: string[] = [];

      for (let i = 0; i < fileList.length; i++) {
        const f = fileList[i];
        if (f.size > MAX_FILE_SIZE) {
          rejected.push(`${f.name} (${formatSize(f.size)})`);
        } else {
          accepted.push({ file: f, name: f.name, size: f.size, type: f.type || "application/octet-stream" });
        }
      }

      if (rejected.length > 0) {
        setError(`Rejected (exceeds 50 MB): ${rejected.join(", ")}`);
      }

      if (accepted.length > 0) {
        onFilesSelected(accepted);
      }
    },
    [onFilesSelected],
  );

  const handleDragEnter = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    dragCounter.current += 1;
    setDragging(true);
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    dragCounter.current -= 1;
    if (dragCounter.current <= 0) {
      dragCounter.current = 0;
      setDragging(false);
    }
  }, []);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      dragCounter.current = 0;
      setDragging(false);
      if (!disabled) processFiles(e.dataTransfer.files);
    },
    [disabled, processFiles],
  );

  return (
    <div className="space-y-2">
      <div
        role="button"
        tabIndex={0}
        onDragEnter={handleDragEnter}
        onDragLeave={handleDragLeave}
        onDragOver={handleDragOver}
        onDrop={handleDrop}
        onClick={() => !disabled && inputRef.current?.click()}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            if (!disabled) inputRef.current?.click();
          }
        }}
        className={`flex flex-col items-center justify-center gap-2 rounded-lg border-2 border-dashed p-8 text-center transition-colors cursor-pointer ${
          disabled
            ? "border-[var(--color-border)] bg-[var(--color-bg-secondary)] opacity-50 cursor-not-allowed"
            : dragging
              ? "border-brand-500 bg-brand-500/5"
              : "border-[var(--color-border)] bg-[var(--color-bg-secondary)] hover:border-brand-400 hover:bg-brand-500/5"
        }`}
      >
        <UploadIcon className="w-8 h-8 text-[var(--color-text-secondary)]" />
        <p className="text-sm text-[var(--color-text-secondary)]">
          Drag & drop files here, or <span className="text-brand-600 dark:text-brand-400 font-medium">browse</span>
        </p>
        <p className="text-xs text-[var(--color-text-secondary)]">Max 50 MB per file</p>
      </div>
      <input
        ref={inputRef}
        type="file"
        multiple
        className="hidden"
        onChange={(e) => {
          processFiles(e.target.files);
          e.target.value = "";
        }}
      />
      {error && (
        <p className="text-xs text-red-600 dark:text-red-400">{error}</p>
      )}
    </div>
  );
}

export function FilePreviewList({
  files,
  onRemove,
}: {
  files: SelectedFile[];
  onRemove: (index: number) => void;
}) {
  if (files.length === 0) return null;

  return (
    <div className="space-y-1.5">
      {files.map((f, i) => (
        <div
          key={`${f.name}-${i}`}
          className="flex items-center gap-3 rounded-md border border-[var(--color-border)] bg-[var(--color-bg-secondary)] px-3 py-2"
        >
          <FileIcon className="w-4 h-4 shrink-0 text-[var(--color-text-secondary)]" />
          <div className="flex-1 min-w-0">
            <p className="text-sm text-[var(--color-text-primary)] truncate">{f.name}</p>
            <p className="text-xs text-[var(--color-text-secondary)]">
              {formatSize(f.size)} &middot; {f.type}
            </p>
          </div>
          <button
            type="button"
            onClick={() => onRemove(i)}
            className="shrink-0 text-[var(--color-text-secondary)] hover:text-red-500 transition-colors"
            aria-label={`Remove ${f.name}`}
          >
            <XIcon className="w-4 h-4" />
          </button>
        </div>
      ))}
    </div>
  );
}

export { formatSize };

// ---------------------------------------------------------------------------
// Icons
// ---------------------------------------------------------------------------

function UploadIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
      <polyline points="17 8 12 3 7 8" />
      <line x1="12" y1="3" x2="12" y2="15" />
    </svg>
  );
}

function FileIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M9 1H4a1 1 0 0 0-1 1v12a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V5L9 1z" />
      <polyline points="9 1 9 5 13 5" />
    </svg>
  );
}

function XIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <line x1="4" y1="4" x2="12" y2="12" />
      <line x1="12" y1="4" x2="4" y2="12" />
    </svg>
  );
}
