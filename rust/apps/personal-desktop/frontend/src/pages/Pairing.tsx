import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useNavigate } from "react-router-dom";
import PeerCard from "../components/PeerCard";

interface PeerInfo {
  device_id: string;
  device_name: string;
  addresses: string[];
  port: number;
  state: string;
  mode: string | null;
}

interface PairedPeer {
  device_id: string;
  device_name: string;
  addresses: string[];
  port: number;
  paired_at: number;
}

function Pairing() {
  const navigate = useNavigate();
  const [discoveredPeers, setDiscoveredPeers] = useState<PeerInfo[]>([]);
  const [pairedPeers, setPairedPeers] = useState<PairedPeer[]>([]);
  const [pairingDeviceId, setPairingDeviceId] = useState<string | null>(null);
  const [otpCode, setOtpCode] = useState<string | null>(null);
  const [manualIp, setManualIp] = useState("");
  const [manualPort, setManualPort] = useState("7010");
  const [error, setError] = useState<string | null>(null);

  const refreshPeers = useCallback(async () => {
    try {
      const discovered = await invoke<PeerInfo[]>("get_discovered_peers");
      setDiscoveredPeers(discovered);
      const paired = await invoke<PairedPeer[]>("get_paired_peers");
      setPairedPeers(paired);
    } catch (err) {
      console.error("Failed to refresh peers:", err);
    }
  }, []);

  useEffect(() => {
    refreshPeers();
    const interval = setInterval(refreshPeers, 3000);
    return () => clearInterval(interval);
  }, [refreshPeers]);

  const handlePair = async (deviceId: string) => {
    setError(null);
    setPairingDeviceId(deviceId);
    try {
      const code = await invoke<string>("initiate_pairing", {
        deviceId,
      });
      setOtpCode(code);
    } catch (err) {
      setError(String(err));
      setPairingDeviceId(null);
    }
  };

  const handleConfirm = async () => {
    if (!pairingDeviceId || !otpCode) return;
    setError(null);
    try {
      await invoke("confirm_pairing", {
        deviceId: pairingDeviceId,
        code: otpCode,
      });
      setPairingDeviceId(null);
      setOtpCode(null);
      await refreshPeers();
    } catch (err) {
      setError(String(err));
    }
  };

  const handleAddManual = async () => {
    if (!manualIp.trim()) return;
    setError(null);
    try {
      await invoke("add_manual_peer", {
        ip: manualIp.trim(),
        port: parseInt(manualPort, 10) || 7010,
      });
      setManualIp("");
      await refreshPeers();
    } catch (err) {
      setError(String(err));
    }
  };

  const handleDismissModal = () => {
    setPairingDeviceId(null);
    setOtpCode(null);
  };

  return (
    <div className="flex flex-col min-h-screen px-6 py-8 max-w-lg mx-auto">
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <button
          onClick={() => navigate("/")}
          className="p-2 rounded-xl text-gray-400 hover:text-gray-600
                     dark:text-gray-500 dark:hover:text-gray-300 transition-colors"
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
          >
            <path d="m15 18-6-6 6-6" />
          </svg>
        </button>
        <h1 className="text-lg font-bold text-gray-800 dark:text-gray-100">
          Pair Devices
        </h1>
        <div className="w-9" />
      </div>

      {/* Error banner */}
      {error && (
        <div className="mb-4 p-3 rounded-xl bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
          <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
        </div>
      )}

      {/* Discovered peers */}
      <section className="mb-8">
        <h2 className="text-sm font-medium text-gray-500 dark:text-gray-400 mb-3 uppercase tracking-wide">
          Nearby Devices
        </h2>
        {discoveredPeers.length === 0 ? (
          <div className="text-center py-8">
            <p className="text-sm text-gray-400 dark:text-gray-500">
              Scanning for devices on your network...
            </p>
            <div className="mt-3 flex justify-center">
              <div className="w-5 h-5 border-2 border-coral border-t-transparent rounded-full animate-spin" />
            </div>
          </div>
        ) : (
          <div className="flex flex-col gap-3">
            {discoveredPeers.map((peer) => (
              <PeerCard
                key={peer.device_id}
                deviceId={peer.device_id}
                deviceName={peer.device_name}
                addresses={peer.addresses}
                port={peer.port}
                onPair={handlePair}
                isPairing={pairingDeviceId === peer.device_id}
              />
            ))}
          </div>
        )}
      </section>

      {/* Paired peers */}
      {pairedPeers.length > 0 && (
        <section className="mb-8">
          <h2 className="text-sm font-medium text-gray-500 dark:text-gray-400 mb-3 uppercase tracking-wide">
            Paired Devices
          </h2>
          <div className="flex flex-col gap-2">
            {pairedPeers.map((peer) => (
              <div
                key={peer.device_id}
                className="flex items-center gap-3 p-3 rounded-xl bg-green-50 dark:bg-green-900/10 border border-green-200 dark:border-green-800"
              >
                <div className="w-2 h-2 rounded-full bg-green-400" />
                <div className="flex-1 min-w-0">
                  <p className="text-sm font-medium text-gray-700 dark:text-gray-200 truncate">
                    {peer.device_name}
                  </p>
                  <p className="text-xs text-gray-400">
                    {peer.addresses[0]}:{peer.port}
                  </p>
                </div>
                <span className="text-xs text-green-600 dark:text-green-400 font-medium">
                  Connected
                </span>
              </div>
            ))}
          </div>
        </section>
      )}

      {/* Manual add */}
      <section className="mt-auto pt-6 border-t border-gray-100 dark:border-gray-700">
        <h2 className="text-sm font-medium text-gray-500 dark:text-gray-400 mb-3">
          Add by IP
        </h2>
        <div className="flex gap-2">
          <input
            type="text"
            placeholder="192.168.1.x"
            value={manualIp}
            onChange={(e) => setManualIp(e.target.value)}
            className="flex-1 px-3 py-2 rounded-xl border border-gray-200 dark:border-gray-600
                       bg-white dark:bg-dark-card text-sm text-gray-800 dark:text-gray-100
                       placeholder-gray-300 dark:placeholder-gray-600
                       focus:outline-none focus:ring-2 focus:ring-coral/30 focus:border-coral"
          />
          <input
            type="text"
            placeholder="7010"
            value={manualPort}
            onChange={(e) => setManualPort(e.target.value)}
            className="w-20 px-3 py-2 rounded-xl border border-gray-200 dark:border-gray-600
                       bg-white dark:bg-dark-card text-sm text-gray-800 dark:text-gray-100
                       placeholder-gray-300 dark:placeholder-gray-600
                       focus:outline-none focus:ring-2 focus:ring-coral/30 focus:border-coral"
          />
          <button
            onClick={handleAddManual}
            className="px-4 py-2 rounded-xl bg-coral text-white text-sm font-medium
                       hover:opacity-90 active:scale-95 transition-all duration-150"
          >
            Add
          </button>
        </div>
      </section>

      {/* OTP Modal */}
      {otpCode && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm">
          <div className="bg-white dark:bg-dark-card rounded-3xl p-8 shadow-2xl max-w-sm w-full mx-4">
            <h3 className="text-lg font-bold text-gray-800 dark:text-gray-100 text-center mb-2">
              Pairing Code
            </h3>
            <p className="text-sm text-gray-500 dark:text-gray-400 text-center mb-6">
              Enter this code on the other device to complete pairing.
            </p>
            <div className="flex justify-center gap-2 mb-8">
              {otpCode.split("").map((digit, i) => (
                <div
                  key={i}
                  className="w-11 h-14 rounded-xl bg-cream dark:bg-gray-800 border border-gray-200 dark:border-gray-600
                             flex items-center justify-center text-2xl font-bold text-coral"
                >
                  {digit}
                </div>
              ))}
            </div>
            <div className="flex gap-3">
              <button
                onClick={handleDismissModal}
                className="flex-1 py-3 rounded-xl border border-gray-200 dark:border-gray-600
                           text-sm font-medium text-gray-600 dark:text-gray-300
                           hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleConfirm}
                className="flex-1 py-3 rounded-xl bg-coral text-white text-sm font-medium
                           hover:opacity-90 active:scale-95 transition-all duration-150"
              >
                Confirm
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default Pairing;
