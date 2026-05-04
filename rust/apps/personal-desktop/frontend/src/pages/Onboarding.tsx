import { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import PeerCard from "../components/PeerCard";

interface DiscoveredPeer {
  deviceId: string;
  deviceName: string;
  addresses: string[];
  port: number;
}

function Onboarding() {
  const navigate = useNavigate();
  const [step, setStep] = useState(1);
  const [peers, setPeers] = useState<DiscoveredPeer[]>([]);
  const [pairingId, setPairingId] = useState<string | null>(null);
  const [scanning, setScanning] = useState(false);

  // Discover peers when entering step 2
  useEffect(() => {
    if (step === 2) {
      discoverPeers();
    }
  }, [step]);

  async function discoverPeers() {
    setScanning(true);
    try {
      const discovered = await invoke<DiscoveredPeer[]>("get_discovered_peers");
      setPeers(discovered);
    } catch {
      setPeers([]);
    } finally {
      setScanning(false);
    }
  }

  async function handlePair(deviceId: string) {
    setPairingId(deviceId);
    try {
      await invoke("initiate_pairing", { deviceId });
      // Pairing succeeded, go to done
      goToStep(3);
    } catch {
      // Still move forward even if pairing fails for now
    } finally {
      setPairingId(null);
    }
  }

  function completeOnboarding() {
    localStorage.setItem("onboarding_completed", "true");
    navigate("/");
  }

  function goToStep(next: number) {
    setStep(next);
  }

  return (
    <div className="flex flex-col items-center justify-center min-h-screen px-6 py-12">
      <div className="relative w-full max-w-[400px]">
        {/* Progress dots */}
        <div className="flex items-center justify-center gap-2 mb-10">
          {[1, 2, 3].map((s) => (
            <div
              key={s}
              className={`
                h-2 rounded-full transition-all duration-300
                ${s === step ? "w-8 bg-coral" : "w-2 bg-gray-200 dark:bg-gray-600"}
              `}
            />
          ))}
        </div>

        {/* Step content with transition */}
        <div
          key={step}
          className={`
            flex flex-col items-center gap-6 w-full
            animate-fade-in
          `}
        >
          {step === 1 && <WelcomeScreen onContinue={() => goToStep(2)} />}
          {step === 2 && (
            <DiscoverScreen
              peers={peers}
              scanning={scanning}
              pairingId={pairingId}
              onPair={handlePair}
              onRefresh={discoverPeers}
              onSkip={() => {
                localStorage.setItem("onboarding_completed", "true");
                navigate("/");
              }}
              onContinue={() => goToStep(3)}
            />
          )}
          {step === 3 && <DoneScreen onFinish={completeOnboarding} />}
        </div>
      </div>
    </div>
  );
}

/* --- Step 1: Welcome --- */
function WelcomeScreen({ onContinue }: { onContinue: () => void }) {
  return (
    <>
      {/* Illustration placeholder */}
      <div className="w-32 h-32 rounded-3xl bg-gradient-to-br from-coral/20 to-mint/20 flex items-center justify-center mb-2">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="48"
          height="48"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
          strokeLinecap="round"
          strokeLinejoin="round"
          className="text-coral"
        >
          <rect x="9" y="2" width="6" height="6" rx="1" />
          <rect x="9" y="16" width="6" height="6" rx="1" />
          <path d="M12 8v8" />
          <path d="M5 12h14" />
          <circle cx="5" cy="12" r="2" />
          <circle cx="19" cy="12" r="2" />
        </svg>
      </div>

      <h1 className="text-3xl font-bold text-gray-800 dark:text-gray-100 text-center leading-tight">
        Copy here, paste anywhere
      </h1>

      <p className="text-center text-gray-500 dark:text-gray-400 leading-relaxed text-lg">
        ClipSync keeps your clipboard in harmony across all your devices.
        No accounts, no cloud — just your local network.
      </p>

      <button
        onClick={onContinue}
        className="mt-6 w-full py-4 bg-coral text-white rounded-2xl font-semibold text-lg
                   hover:opacity-90 active:scale-[0.98] transition-all duration-150
                   shadow-lg shadow-coral/20"
      >
        Get Started
      </button>
    </>
  );
}

/* --- Step 2: Discover --- */
function DiscoverScreen({
  peers,
  scanning,
  pairingId,
  onPair,
  onRefresh,
  onSkip,
  onContinue,
}: {
  peers: DiscoveredPeer[];
  scanning: boolean;
  pairingId: string | null;
  onPair: (id: string) => void;
  onRefresh: () => void;
  onSkip: () => void;
  onContinue: () => void;
}) {
  return (
    <>
      <div className="w-16 h-16 rounded-2xl bg-mint/15 flex items-center justify-center">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="28"
          height="28"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          className="text-mint"
        >
          <circle cx="12" cy="12" r="2" />
          <path d="M16.24 7.76a6 6 0 0 1 0 8.49" />
          <path d="M7.76 16.24a6 6 0 0 1 0-8.49" />
          <path d="M19.07 4.93a10 10 0 0 1 0 14.14" />
          <path d="M4.93 19.07a10 10 0 0 1 0-14.14" />
        </svg>
      </div>

      <h2 className="text-2xl font-bold text-gray-800 dark:text-gray-100 text-center">
        Looking for devices...
      </h2>

      <p className="text-center text-gray-500 dark:text-gray-400 leading-relaxed">
        We found these devices on your network. Tap Pair to connect.
      </p>

      {/* Peer list */}
      <div className="w-full flex flex-col gap-3 mt-2 max-h-[240px] overflow-y-auto">
        {scanning && peers.length === 0 && (
          <div className="flex flex-col items-center gap-3 py-8">
            <div className="w-6 h-6 border-2 border-coral border-t-transparent rounded-full animate-spin" />
            <p className="text-sm text-gray-400">Scanning your network...</p>
          </div>
        )}

        {!scanning && peers.length === 0 && (
          <div className="flex flex-col items-center gap-3 py-8">
            <p className="text-sm text-gray-400 text-center">
              No devices found yet. Make sure ClipSync is running on another device.
            </p>
            <button
              onClick={onRefresh}
              className="text-sm text-coral font-medium hover:underline"
            >
              Scan again
            </button>
          </div>
        )}

        {peers.map((peer) => (
          <PeerCard
            key={peer.deviceId}
            deviceId={peer.deviceId}
            deviceName={peer.deviceName}
            addresses={peer.addresses}
            port={peer.port}
            onPair={onPair}
            isPairing={pairingId === peer.deviceId}
          />
        ))}
      </div>

      {peers.length > 0 && (
        <button
          onClick={onContinue}
          className="mt-4 w-full py-4 bg-coral text-white rounded-2xl font-semibold text-lg
                     hover:opacity-90 active:scale-[0.98] transition-all duration-150
                     shadow-lg shadow-coral/20"
        >
          Continue
        </button>
      )}

      <button
        onClick={onSkip}
        className="mt-2 text-sm text-gray-400 hover:text-gray-600 dark:hover:text-gray-300
                   transition-colors duration-150"
      >
        Skip — I'll do it later
      </button>
    </>
  );
}

/* --- Step 3: Done --- */
function DoneScreen({ onFinish }: { onFinish: () => void }) {
  return (
    <>
      {/* Checkmark animation */}
      <div className="w-24 h-24 rounded-full bg-mint/15 flex items-center justify-center animate-check-pop">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="40"
          height="40"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2.5"
          strokeLinecap="round"
          strokeLinejoin="round"
          className="text-mint animate-check-draw"
        >
          <polyline points="20 6 9 17 4 12" />
        </svg>
      </div>

      <h2 className="text-2xl font-bold text-gray-800 dark:text-gray-100 text-center">
        You're all set!
      </h2>

      <p className="text-center text-gray-500 dark:text-gray-400 leading-relaxed">
        ClipSync lives in your menu bar (the little clipboard icon up top).
        It'll quietly sync in the background — no need to keep this window open.
      </p>

      <div className="w-full p-4 rounded-2xl bg-mint/10 border border-mint/20 mt-2">
        <p className="text-sm text-gray-600 dark:text-gray-300 text-center">
          <span className="font-medium">Tip:</span> Right-click the tray icon for quick settings and recent clips.
        </p>
      </div>

      <button
        onClick={onFinish}
        className="mt-6 w-full py-4 bg-coral text-white rounded-2xl font-semibold text-lg
                   hover:opacity-90 active:scale-[0.98] transition-all duration-150
                   shadow-lg shadow-coral/20"
      >
        Open ClipSync
      </button>
    </>
  );
}

export default Onboarding;
