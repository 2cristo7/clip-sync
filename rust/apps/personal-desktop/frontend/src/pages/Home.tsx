import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import DeviceCard, { PairedDevice } from "../components/DeviceCard";
import SendFile, { ConnectedPeer } from "../components/SendFile";

function Home() {
  const navigate = useNavigate();
  const [syncOn, setSyncOn] = useState(true);
  const [devices, setDevices] = useState<PairedDevice[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadData();
  }, []);

  async function loadData() {
    try {
      const [status, peers] = await Promise.all([
        invoke<string>("get_sync_status"),
        invoke<PairedDevice[]>("get_paired_peers"),
      ]);
      setSyncOn(status !== "paused");
      setDevices(peers);
    } catch {
      // Fallback: keep defaults
    } finally {
      setLoading(false);
    }
  }

  async function handleTogglePause() {
    try {
      const isPaused = await invoke<boolean>("cmd_toggle_pause");
      setSyncOn(!isPaused);
    } catch {
      // noop
    }
  }

  async function handleForget(deviceId: string) {
    try {
      await invoke("forget_peer", { deviceId });
      setDevices((prev) => prev.filter((d) => d.device_id !== deviceId));
    } catch {
      // noop
    }
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center h-screen">
        <div className="w-6 h-6 border-2 border-coral border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  // Build peer list for SendFile component
  const connectedPeers: ConnectedPeer[] = devices.map((d) => ({
    device_id: d.device_id,
    device_name: d.device_name,
    is_online: d.is_online ?? false,
  }));

  return (
    <div className="relative flex flex-col min-h-screen p-6 max-w-md mx-auto">
      {/* File drop overlay */}
      <SendFile peers={connectedPeers} />
      {/* Header */}
      <div className="flex items-center justify-between mb-8">
        <h1 className="text-lg font-semibold text-gray-800 dark:text-gray-100">
          ClipSync
        </h1>
        <button
          onClick={() => navigate("/advanced")}
          className="p-2 rounded-lg hover:bg-gray-100 dark:hover:bg-dark-surface transition-colors"
          aria-label="Settings"
        >
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
            className="text-gray-500 dark:text-gray-400"
          >
            <circle cx="12" cy="12" r="3" />
            <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
          </svg>
        </button>
      </div>

      {/* Master pause toggle */}
      <div className="flex items-center justify-between p-5 rounded-2xl bg-white dark:bg-dark-surface border border-gray-100 dark:border-gray-700 mb-8">
        <div>
          <p className="text-sm font-semibold text-gray-800 dark:text-gray-100">
            {syncOn ? "Sync is on" : "Sync is paused"}
          </p>
          <p className="text-xs text-gray-400 dark:text-gray-500 mt-0.5">
            {syncOn
              ? "Clipboard shared across devices"
              : "Clipboard sharing paused"}
          </p>
        </div>
        <button
          onClick={handleTogglePause}
          className={`
            relative w-12 h-7 rounded-full transition-colors duration-200
            ${syncOn ? "bg-coral" : "bg-gray-300 dark:bg-gray-600"}
          `}
          aria-label="Toggle sync"
        >
          <span
            className={`
              absolute top-1 w-5 h-5 rounded-full bg-white shadow-sm
              transition-transform duration-200
              ${syncOn ? "translate-x-6" : "translate-x-1"}
            `}
          />
        </button>
      </div>

      {/* Device list */}
      <div className="flex-1">
        <h2 className="text-xs font-medium text-gray-400 dark:text-gray-500 uppercase tracking-wide mb-3">
          Paired Devices
        </h2>

        {devices.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-16 text-center">
            <div className="w-16 h-16 rounded-full bg-coral/10 flex items-center justify-center mb-4">
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
                <path d="M5 12h14" />
                <path d="M12 5v14" />
              </svg>
            </div>
            <p className="text-sm font-medium text-gray-600 dark:text-gray-300 mb-1">
              No devices yet
            </p>
            <p className="text-xs text-gray-400 dark:text-gray-500 mb-4">
              Let's add one!
            </p>
            <button
              onClick={() => navigate("/pairing")}
              className="px-5 py-2.5 rounded-xl text-sm font-medium border-2 border-coral text-coral hover:bg-coral hover:text-white transition-colors duration-150"
            >
              Add device
            </button>
          </div>
        ) : (
          <div className="space-y-3">
            {devices.map((device) => (
              <DeviceCard
                key={device.device_id}
                device={device}
                onForget={handleForget}
              />
            ))}
          </div>
        )}
      </div>

      {/* Add device button (when devices exist) */}
      {devices.length > 0 && (
        <div className="pt-6 pb-2">
          <button
            onClick={() => navigate("/pairing")}
            className="w-full py-3 rounded-xl text-sm font-medium border-2 border-coral text-coral hover:bg-coral hover:text-white transition-colors duration-150"
          >
            Add device
          </button>
        </div>
      )}
    </div>
  );
}

export default Home;
