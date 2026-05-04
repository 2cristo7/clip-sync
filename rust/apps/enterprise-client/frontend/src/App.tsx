import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface ClipItem {
  text: string;
  timestamp: number;
}

export function App() {
  const [clips, setClips] = useState<ClipItem[]>([]);
  const [status, setStatus] = useState("disconnected");
  const [policy, setPolicy] = useState("ReadWrite");
  const [paused, setPaused] = useState(false);

  useEffect(() => {
    async function load() {
      const s = await invoke<string>("get_connection_status");
      setStatus(s);
      const p = await invoke<string>("get_policy_mode");
      setPolicy(p);
      const sp = await invoke<boolean>("get_sync_paused");
      setPaused(sp);
      const c = await invoke<ClipItem[]>("get_recent_clips");
      setClips(c);
    }
    load();
    const interval = setInterval(load, 3000);
    return () => clearInterval(interval);
  }, []);

  const handleToggleSync = async () => {
    const newPaused = await invoke<boolean>("toggle_sync");
    setPaused(newPaused);
  };

  return (
    <div className="flex flex-col h-screen bg-[var(--color-bg-primary)]">
      <header className="flex items-center justify-between px-4 py-3 border-b border-[var(--color-border)]">
        <h1 className="text-sm font-semibold text-[var(--color-text-primary)]">
          Recent Clips
        </h1>
        <div className="flex items-center gap-3">
          <span
            className={`inline-block w-2 h-2 rounded-full ${
              status === "connected" ? "bg-green-500" : "bg-red-500"
            }`}
          />
          <span className="text-xs text-[var(--color-text-secondary)]">
            {policy}
          </span>
          <button
            onClick={handleToggleSync}
            className={`text-xs px-2 py-1 rounded ${
              paused
                ? "bg-brand-500 text-white"
                : "bg-gray-200 dark:bg-gray-800 text-[var(--color-text-primary)]"
            }`}
          >
            {paused ? "Resume" : "Pause"}
          </button>
        </div>
      </header>
      <main className="flex-1 overflow-y-auto">
        {clips.length === 0 ? (
          <div className="flex items-center justify-center h-full text-sm text-[var(--color-text-secondary)]">
            No clips yet
          </div>
        ) : (
          <ul className="divide-y divide-[var(--color-border)]">
            {clips.map((clip, i) => (
              <li key={i} className="px-4 py-3">
                <p className="text-sm text-[var(--color-text-primary)] line-clamp-3 font-mono">
                  {clip.text}
                </p>
                <time className="text-xs text-[var(--color-text-secondary)] mt-1 block">
                  {new Date(clip.timestamp * 1000).toLocaleString()}
                </time>
              </li>
            ))}
          </ul>
        )}
      </main>
    </div>
  );
}
