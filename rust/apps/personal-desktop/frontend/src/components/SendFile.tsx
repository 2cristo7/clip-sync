import { useState, useCallback, DragEvent } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface ConnectedPeer {
  device_id: string;
  device_name: string;
  is_online: boolean;
}

interface SendFileProps {
  peers: ConnectedPeer[];
}

interface DroppedFile {
  name: string;
  path: string;
  size: number;
}

const MAX_FILE_SIZE = 50 * 1024 * 1024; // 50 MB

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function SendFile({ peers }: SendFileProps) {
  const [isDragging, setIsDragging] = useState(false);
  const [droppedFile, setDroppedFile] = useState<DroppedFile | null>(null);
  const [selectedPeers, setSelectedPeers] = useState<Set<string>>(new Set());
  const [sending, setSending] = useState(false);
  const [sent, setSent] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tooLarge, setTooLarge] = useState(false);

  const onlinePeers = peers.filter((p) => p.is_online);

  const handleDragEnter = useCallback((e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(true);
  }, []);

  const handleDragLeave = useCallback((e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    // Only hide if leaving the container itself
    if (e.currentTarget === e.target) {
      setIsDragging(false);
    }
  }, []);

  const handleDragOver = useCallback((e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
  }, []);

  const handleDrop = useCallback((e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(false);
    setError(null);
    setSent(false);

    const files = e.dataTransfer?.files;
    if (!files || files.length === 0) return;

    const file = files[0];

    if (file.size > MAX_FILE_SIZE) {
      setTooLarge(true);
      setDroppedFile({ name: file.name, path: "", size: file.size });
      return;
    }

    setTooLarge(false);
    // Tauri provides the file path via dataTransfer
    const path = (file as unknown as { path?: string }).path || file.name;
    setDroppedFile({ name: file.name, path, size: file.size });
    // Auto-select all online peers
    setSelectedPeers(new Set(onlinePeers.map((p) => p.device_id)));
  }, [onlinePeers]);

  const togglePeer = (deviceId: string) => {
    setSelectedPeers((prev) => {
      const next = new Set(prev);
      if (next.has(deviceId)) {
        next.delete(deviceId);
      } else {
        next.add(deviceId);
      }
      return next;
    });
  };

  const selectAll = () => {
    setSelectedPeers(new Set(onlinePeers.map((p) => p.device_id)));
  };

  const handleSend = async () => {
    if (!droppedFile || selectedPeers.size === 0) return;

    setSending(true);
    setError(null);

    try {
      await invoke("send_file", {
        filePath: droppedFile.path,
        peerIds: Array.from(selectedPeers),
      });
      setSent(true);
      setTimeout(() => {
        handleCancel();
      }, 2000);
    } catch (err) {
      setError(String(err));
    } finally {
      setSending(false);
    }
  };

  const handleCancel = () => {
    setDroppedFile(null);
    setSelectedPeers(new Set());
    setSending(false);
    setSent(false);
    setError(null);
    setTooLarge(false);
  };

  // Drag overlay (always rendered, visible on drag)
  if (!droppedFile && !isDragging) {
    return (
      <div
        className="absolute inset-0 z-50 pointer-events-auto opacity-0"
        onDragEnter={handleDragEnter}
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
      />
    );
  }

  // Drag hover state
  if (isDragging && !droppedFile) {
    return (
      <div
        className="absolute inset-0 z-50 flex items-center justify-center bg-coral/5 backdrop-blur-sm border-2 border-dashed border-coral rounded-2xl transition-all"
        onDragEnter={handleDragEnter}
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
      >
        <div className="text-center">
          <div className="w-16 h-16 mx-auto rounded-full bg-coral/10 flex items-center justify-center mb-4">
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="28"
              height="28"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.5"
              strokeLinecap="round"
              strokeLinejoin="round"
              className="text-coral"
            >
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
              <polyline points="17 8 12 3 7 8" />
              <line x1="12" y1="3" x2="12" y2="15" />
            </svg>
          </div>
          <p className="text-sm font-medium text-gray-700 dark:text-gray-200">
            Drop file to send
          </p>
          <p className="text-xs text-gray-400 dark:text-gray-500 mt-1">
            Max 50 MB
          </p>
        </div>
      </div>
    );
  }

  // File too large rejection
  if (tooLarge && droppedFile) {
    return (
      <div className="absolute inset-0 z-50 flex items-center justify-center bg-white/95 dark:bg-dark-surface/95 backdrop-blur-sm rounded-2xl">
        <div className="text-center p-6 max-w-xs">
          <div className="w-16 h-16 mx-auto rounded-full bg-red-50 dark:bg-red-900/20 flex items-center justify-center mb-4">
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="28"
              height="28"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.5"
              strokeLinecap="round"
              strokeLinejoin="round"
              className="text-red-400"
            >
              <circle cx="12" cy="12" r="10" />
              <line x1="15" y1="9" x2="9" y2="15" />
              <line x1="9" y1="9" x2="15" y2="15" />
            </svg>
          </div>
          <p className="text-sm font-semibold text-gray-800 dark:text-gray-100 mb-1">
            File too big
          </p>
          <p className="text-xs text-gray-400 dark:text-gray-500 mb-1">
            {droppedFile.name} ({formatSize(droppedFile.size)})
          </p>
          <p className="text-xs text-gray-400 dark:text-gray-500 mb-4">
            Try sharing via email instead
          </p>
          <button
            onClick={handleCancel}
            className="px-5 py-2 rounded-xl text-sm font-medium bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-200 hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors"
          >
            Dismiss
          </button>
        </div>
      </div>
    );
  }

  // Peer selector + send
  return (
    <div className="absolute inset-0 z-50 flex flex-col bg-white/95 dark:bg-dark-surface/95 backdrop-blur-sm rounded-2xl p-6">
      {/* File info */}
      <div className="flex items-center gap-3 mb-6 p-4 rounded-xl bg-gray-50 dark:bg-gray-800">
        <div className="w-10 h-10 rounded-lg bg-coral/10 flex items-center justify-center flex-shrink-0">
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="18"
            height="18"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            className="text-coral"
          >
            <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
            <polyline points="14 2 14 8 20 8" />
          </svg>
        </div>
        <div className="min-w-0 flex-1">
          <p className="text-sm font-medium text-gray-800 dark:text-gray-100 truncate">
            {droppedFile?.name}
          </p>
          <p className="text-xs text-gray-400 dark:text-gray-500">
            {formatSize(droppedFile?.size ?? 0)}
          </p>
        </div>
      </div>

      {/* Peer selector */}
      <div className="flex items-center justify-between mb-3">
        <h3 className="text-xs font-medium text-gray-400 dark:text-gray-500 uppercase tracking-wide">
          Send to
        </h3>
        {onlinePeers.length > 1 && (
          <button
            onClick={selectAll}
            className="text-xs font-medium text-coral hover:text-coral/80 transition-colors"
          >
            Select all
          </button>
        )}
      </div>

      <div className="flex-1 overflow-y-auto space-y-2 mb-6">
        {onlinePeers.length === 0 ? (
          <p className="text-xs text-gray-400 dark:text-gray-500 text-center py-4">
            No peers online
          </p>
        ) : (
          onlinePeers.map((peer) => (
            <label
              key={peer.device_id}
              className="flex items-center gap-3 p-3 rounded-xl hover:bg-gray-50 dark:hover:bg-gray-800 cursor-pointer transition-colors"
            >
              <input
                type="checkbox"
                checked={selectedPeers.has(peer.device_id)}
                onChange={() => togglePeer(peer.device_id)}
                className="w-4 h-4 rounded border-gray-300 text-coral focus:ring-coral"
              />
              <div className="flex items-center gap-2">
                <span className="w-2 h-2 rounded-full bg-green-400" />
                <span className="text-sm text-gray-700 dark:text-gray-200">
                  {peer.device_name}
                </span>
              </div>
            </label>
          ))
        )}
      </div>

      {/* Error */}
      {error && (
        <p className="text-xs text-red-400 mb-3 text-center">{error}</p>
      )}

      {/* Success */}
      {sent && (
        <p className="text-xs text-green-500 mb-3 text-center">
          File sent successfully!
        </p>
      )}

      {/* Actions */}
      <div className="flex gap-3">
        <button
          onClick={handleCancel}
          className="flex-1 py-2.5 rounded-xl text-sm font-medium bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-200 hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors"
        >
          Cancel
        </button>
        <button
          onClick={handleSend}
          disabled={sending || selectedPeers.size === 0}
          className={`
            flex-1 py-2.5 rounded-xl text-sm font-medium text-white transition-colors
            ${
              sending || selectedPeers.size === 0
                ? "bg-coral/50 cursor-not-allowed"
                : "bg-coral hover:bg-coral/90"
            }
          `}
        >
          {sending ? "Sending..." : `Send to ${selectedPeers.size} device${selectedPeers.size !== 1 ? "s" : ""}`}
        </button>
      </div>
    </div>
  );
}

export default SendFile;
