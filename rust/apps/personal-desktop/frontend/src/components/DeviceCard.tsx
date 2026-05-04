import { useState } from "react";

export interface PairedDevice {
  device_id: string;
  device_name: string;
  addresses: string[];
  port: number;
  shared_secret: string;
  paired_at: number;
  is_online?: boolean;
  last_sync?: number;
}

interface DeviceCardProps {
  device: PairedDevice;
  onForget: (deviceId: string) => void;
}

function timeAgo(timestamp: number): string {
  if (!timestamp) return "Never synced";
  const seconds = Math.floor((Date.now() / 1000) - timestamp);
  if (seconds < 60) return "Synced just now";
  if (seconds < 3600) return `Synced ${Math.floor(seconds / 60)} min ago`;
  if (seconds < 86400) return `Synced ${Math.floor(seconds / 3600)}h ago`;
  return `Synced ${Math.floor(seconds / 86400)}d ago`;
}

function DeviceCard({ device, onForget }: DeviceCardProps) {
  const [hovered, setHovered] = useState(false);
  const [confirmForget, setConfirmForget] = useState(false);

  const isOnline = device.is_online ?? false;
  const statusText = isOnline ? timeAgo(device.last_sync ?? 0) : "Offline";

  const handleForget = () => {
    if (confirmForget) {
      onForget(device.device_id);
      setConfirmForget(false);
    } else {
      setConfirmForget(true);
    }
  };

  return (
    <div
      className={`
        relative p-5 rounded-2xl border transition-all duration-200
        bg-white dark:bg-dark-surface
        border-gray-100 dark:border-gray-700
        ${hovered ? "shadow-md" : "shadow-sm"}
      `}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => {
        setHovered(false);
        setConfirmForget(false);
      }}
    >
      <div className="flex items-center gap-4">
        {/* Device icon */}
        <div className="flex-shrink-0 w-10 h-10 rounded-xl bg-coral/10 flex items-center justify-center">
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="20"
            height="20"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            className="text-coral"
          >
            <rect x="2" y="3" width="20" height="14" rx="2" />
            <line x1="8" y1="21" x2="16" y2="21" />
            <line x1="12" y1="17" x2="12" y2="21" />
          </svg>
        </div>

        {/* Info */}
        <div className="flex-1 min-w-0">
          <h3 className="text-sm font-semibold text-gray-800 dark:text-gray-100 truncate">
            {device.device_name}
          </h3>
          <div className="flex items-center gap-1.5 mt-1">
            <span
              className={`w-2 h-2 rounded-full ${
                isOnline ? "bg-green-400" : "bg-gray-300 dark:bg-gray-600"
              }`}
            />
            <span className="text-xs text-gray-400 dark:text-gray-500">
              {statusText}
            </span>
          </div>
        </div>

        {/* Forget action (visible on hover) */}
        {hovered && (
          <button
            onClick={handleForget}
            className="flex-shrink-0 text-xs font-medium text-red-400 hover:text-red-500 transition-colors"
          >
            {confirmForget ? "Confirm?" : "Forget"}
          </button>
        )}
      </div>
    </div>
  );
}

export default DeviceCard;
