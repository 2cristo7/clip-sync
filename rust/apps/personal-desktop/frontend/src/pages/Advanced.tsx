import { useState, useEffect, useRef, useCallback } from "react";
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { toggleTheme } from "../theme";

// ── Types ────────────────────────────────────────────────────────

interface DeviceSettings {
  direction: "both" | "send_only" | "receive_only";
}

interface Settings {
  clipboard: { text: boolean; image: boolean; files: boolean };
  notifications: { toast_on_receive: boolean; sound: boolean };
  autostart: boolean;
  network: { tailscale_hostname: string | null };
  devices: Record<string, DeviceSettings>;
}

// ── Toggle component ─────────────────────────────────────────────

function Toggle({
  checked,
  onChange,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      onClick={() => onChange(!checked)}
      className={`relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none ${
        checked ? "bg-coral" : "bg-gray-300 dark:bg-gray-600"
      }`}
    >
      <span
        className={`pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out ${
          checked ? "translate-x-5" : "translate-x-0"
        }`}
      />
    </button>
  );
}

// ── Section divider ──────────────────────────────────────────────

function SectionHeader({ title }: { title: string }) {
  return (
    <h3 className="text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-gray-400 mt-6 mb-2 px-1">
      {title}
    </h3>
  );
}

// ── Main component ───────────────────────────────────────────────

function Advanced() {
  const navigate = useNavigate();
  const [settings, setSettings] = useState<Settings | null>(null);
  const [debugLogs, setDebugLogs] = useState<string[]>([]);
  const [debugOpen, setDebugOpen] = useState(false);
  const [confirmReset, setConfirmReset] = useState(false);
  const [tailscaleInput, setTailscaleInput] = useState("");
  const logEndRef = useRef<HTMLDivElement>(null);

  // Load settings on mount
  useEffect(() => {
    invoke<Settings>("get_settings").then((s) => {
      setSettings(s);
      setTailscaleInput(s.network.tailscale_hostname ?? "");
    });
  }, []);

  // Auto-scroll log viewer
  useEffect(() => {
    if (debugOpen) {
      logEndRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, [debugLogs, debugOpen]);

  const updateSetting = useCallback(
    async (key: string, value: string) => {
      await invoke("update_setting", { key, value });
      const updated = await invoke<Settings>("get_settings");
      setSettings(updated);
    },
    []
  );

  const handleReset = async () => {
    await invoke("reset_all");
    setConfirmReset(false);
    navigate("/onboarding", { replace: true });
  };

  const loadLogs = async () => {
    const logs = await invoke<string[]>("get_debug_log");
    setDebugLogs(logs);
    setDebugOpen(true);
  };

  if (!settings) {
    return (
      <div className="flex items-center justify-center min-h-screen">
        <div className="animate-pulse text-gray-400">Loading...</div>
      </div>
    );
  }

  return (
    <div className="flex flex-col min-h-screen px-6 py-8 overflow-y-auto">
      {/* Header with back arrow */}
      <div className="flex items-center gap-3 mb-4">
        <button
          onClick={() => navigate("/")}
          className="p-2 rounded-xl text-gray-400 hover:text-gray-600
                     dark:text-gray-500 dark:hover:text-gray-300 transition-colors"
          aria-label="Go back"
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
            <path d="m12 19-7-7 7-7" />
            <path d="M19 12H5" />
          </svg>
        </button>
        <h2 className="text-xl font-bold text-gray-800 dark:text-gray-100">
          Advanced Settings
        </h2>
      </div>

      {/* Per-device toggles */}
      {Object.keys(settings.devices).length > 0 && (
        <>
          <SectionHeader title="Devices" />
          <div className="flex flex-col gap-2">
            {Object.entries(settings.devices).map(([id, dev]) => (
              <div
                key={id}
                className="p-3 rounded-2xl bg-white dark:bg-dark-surface flex items-center justify-between"
              >
                <span className="text-sm font-medium text-gray-700 dark:text-gray-300 truncate max-w-[140px]">
                  {id.slice(0, 8)}...
                </span>
                <select
                  value={dev.direction}
                  onChange={(e) =>
                    updateSetting(`devices.${id}.direction`, e.target.value)
                  }
                  className="text-xs rounded-lg border border-gray-200 dark:border-gray-600 bg-gray-50 dark:bg-dark-bg text-gray-700 dark:text-gray-300 px-2 py-1"
                >
                  <option value="both">Both</option>
                  <option value="send_only">Send only</option>
                  <option value="receive_only">Receive only</option>
                </select>
              </div>
            ))}
          </div>
        </>
      )}

      {/* Clipboard kinds */}
      <SectionHeader title="Clipboard" />
      <div className="flex flex-col gap-1 p-3 rounded-2xl bg-white dark:bg-dark-surface">
        <div className="flex items-center justify-between py-1.5">
          <span className="text-sm text-gray-700 dark:text-gray-300">Text</span>
          <Toggle
            checked={settings.clipboard.text}
            onChange={(v) => updateSetting("clipboard.text", String(v))}
          />
        </div>
        <div className="flex items-center justify-between py-1.5">
          <span className="text-sm text-gray-700 dark:text-gray-300">Images</span>
          <Toggle
            checked={settings.clipboard.image}
            onChange={(v) => updateSetting("clipboard.image", String(v))}
          />
        </div>
        <div className="flex items-center justify-between py-1.5">
          <span className="text-sm text-gray-700 dark:text-gray-300">Files</span>
          <Toggle
            checked={settings.clipboard.files}
            onChange={(v) => updateSetting("clipboard.files", String(v))}
          />
        </div>
      </div>

      {/* Notifications */}
      <SectionHeader title="Notifications" />
      <div className="flex flex-col gap-1 p-3 rounded-2xl bg-white dark:bg-dark-surface">
        <div className="flex items-center justify-between py-1.5">
          <span className="text-sm text-gray-700 dark:text-gray-300">
            Toast on receive
          </span>
          <Toggle
            checked={settings.notifications.toast_on_receive}
            onChange={(v) =>
              updateSetting("notifications.toast_on_receive", String(v))
            }
          />
        </div>
        <div className="flex items-center justify-between py-1.5">
          <span className="text-sm text-gray-700 dark:text-gray-300">Sound</span>
          <Toggle
            checked={settings.notifications.sound}
            onChange={(v) => updateSetting("notifications.sound", String(v))}
          />
        </div>
      </div>

      {/* Autostart */}
      <SectionHeader title="Startup" />
      <div className="p-3 rounded-2xl bg-white dark:bg-dark-surface">
        <div className="flex items-center justify-between py-1.5">
          <span className="text-sm text-gray-700 dark:text-gray-300">
            Launch on login
          </span>
          <Toggle
            checked={settings.autostart}
            onChange={(v) => updateSetting("autostart", String(v))}
          />
        </div>
      </div>

      {/* Appearance */}
      <SectionHeader title="Appearance" />
      <div className="p-3 rounded-2xl bg-white dark:bg-dark-surface">
        <div className="flex items-center justify-between py-1.5">
          <span className="text-sm text-gray-700 dark:text-gray-300">
            Dark Mode
          </span>
          <button
            onClick={toggleTheme}
            className="px-3 py-1 text-xs rounded-lg bg-gray-100 dark:bg-dark-bg
                       text-gray-600 dark:text-gray-400 hover:opacity-80 transition-opacity"
          >
            Toggle
          </button>
        </div>
      </div>

      {/* Network */}
      <SectionHeader title="Network" />
      <div className="p-3 rounded-2xl bg-white dark:bg-dark-surface">
        <label className="text-sm text-gray-700 dark:text-gray-300 block mb-2">
          Tailscale fallback hostname
        </label>
        <input
          type="text"
          value={tailscaleInput}
          onChange={(e) => setTailscaleInput(e.target.value)}
          onBlur={() =>
            updateSetting("network.tailscale_hostname", tailscaleInput)
          }
          placeholder="e.g. my-machine.tailnet.ts.net"
          className="w-full text-sm px-3 py-2 rounded-lg border border-gray-200 dark:border-gray-600 bg-gray-50 dark:bg-dark-bg text-gray-700 dark:text-gray-300 placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-coral/40"
        />
      </div>

      {/* Debug */}
      <SectionHeader title="Debug" />
      <div className="p-3 rounded-2xl bg-white dark:bg-dark-surface">
        <button
          onClick={debugOpen ? () => setDebugOpen(false) : loadLogs}
          className="text-sm text-coral hover:underline"
        >
          {debugOpen ? "Hide log viewer" : "Show log viewer"}
        </button>
        {debugOpen && (
          <div className="mt-3 max-h-60 overflow-y-auto rounded-lg bg-gray-900 p-3">
            <pre className="text-xs font-mono text-green-300 whitespace-pre-wrap">
              {debugLogs.length > 0
                ? debugLogs.join("\n")
                : "No logs available."}
            </pre>
            <div ref={logEndRef} />
          </div>
        )}
      </div>

      {/* Danger zone */}
      <SectionHeader title="Danger Zone" />
      <div className="p-4 rounded-2xl bg-red-50 dark:bg-red-950/30 border border-red-200 dark:border-red-800 mb-8">
        {!confirmReset ? (
          <button
            onClick={() => setConfirmReset(true)}
            className="w-full py-2.5 text-sm font-semibold text-white bg-red-500 hover:bg-red-600 rounded-xl transition-colors"
          >
            Reset &amp; re-pair everything
          </button>
        ) : (
          <div className="flex flex-col gap-3">
            <p className="text-sm text-red-700 dark:text-red-300 text-center">
              This will delete all settings and paired devices. Are you sure?
            </p>
            <div className="flex gap-2">
              <button
                onClick={() => setConfirmReset(false)}
                className="flex-1 py-2 text-sm font-medium text-gray-600 dark:text-gray-300 bg-gray-100 dark:bg-dark-surface rounded-xl hover:opacity-80 transition-opacity"
              >
                Cancel
              </button>
              <button
                onClick={handleReset}
                className="flex-1 py-2 text-sm font-semibold text-white bg-red-600 hover:bg-red-700 rounded-xl transition-colors"
              >
                Confirm reset
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

export default Advanced;
