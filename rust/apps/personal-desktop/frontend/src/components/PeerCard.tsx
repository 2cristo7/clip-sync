import { useState } from "react";

interface PeerCardProps {
  deviceId: string;
  deviceName: string;
  addresses: string[];
  port: number;
  onPair: (deviceId: string) => void;
  isPairing?: boolean;
}

function PeerCard({
  deviceId,
  deviceName,
  addresses,
  port,
  onPair,
  isPairing = false,
}: PeerCardProps) {
  const [hovered, setHovered] = useState(false);

  const displayAddress = addresses.length > 0 ? addresses[0] : "unknown";
  const shortId = deviceId.length > 8 ? deviceId.slice(0, 8) : deviceId;

  return (
    <div
      className={`
        relative p-5 rounded-2xl border transition-all duration-200
        bg-white dark:bg-dark-card
        border-gray-100 dark:border-gray-700
        ${hovered ? "shadow-lg scale-[1.02]" : "shadow-sm"}
      `}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      {/* Device icon */}
      <div className="flex items-start gap-4">
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
            {deviceName}
          </h3>
          <p className="text-xs text-gray-400 dark:text-gray-500 mt-0.5">
            {displayAddress}:{port}
          </p>
          <p className="text-xs text-gray-300 dark:text-gray-600 mt-0.5 font-mono">
            {shortId}...
          </p>
        </div>

        {/* Pair button */}
        <button
          onClick={() => onPair(deviceId)}
          disabled={isPairing}
          className={`
            flex-shrink-0 px-4 py-2 rounded-xl text-sm font-medium
            transition-all duration-150
            ${
              isPairing
                ? "bg-gray-100 dark:bg-gray-700 text-gray-400 cursor-not-allowed"
                : "bg-coral text-white hover:opacity-90 active:scale-95"
            }
          `}
        >
          {isPairing ? "Pairing..." : "Pair"}
        </button>
      </div>
    </div>
  );
}

export default PeerCard;
