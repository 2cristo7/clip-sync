/**
 * Minimal theme toggle utility.
 * Reads/writes the `dark` class on <html> and persists the choice
 * in localStorage under the key `clipsync-theme`.
 */

type Theme = "light" | "dark" | "system";

const STORAGE_KEY = "clipsync-theme";

function getSystemPreference(): "light" | "dark" {
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

function applyTheme(theme: Theme): void {
  const resolved = theme === "system" ? getSystemPreference() : theme;
  document.documentElement.classList.toggle("dark", resolved === "dark");
}

export function initTheme(): void {
  const stored = localStorage.getItem(STORAGE_KEY) as Theme | null;
  applyTheme(stored ?? "system");
}

export function toggleTheme(): void {
  const isDark = document.documentElement.classList.contains("dark");
  const next: Theme = isDark ? "light" : "dark";
  localStorage.setItem(STORAGE_KEY, next);
  applyTheme(next);
}

// Auto-init on module load
initTheme();
