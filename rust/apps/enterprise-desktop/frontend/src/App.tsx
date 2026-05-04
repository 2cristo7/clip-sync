import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { Sidebar } from "./components/Sidebar";
import { TopBar } from "./components/TopBar";
import { useConnectionStatus } from "./hooks/useConnectionStatus";
import { DevicesPage } from "./pages/Devices";
import { AuditPage } from "./pages/Audit";
import { BroadcastPage } from "./pages/Broadcast";
import { SettingsPage } from "./pages/Settings";

export function App() {
  const connectionStatus = useConnectionStatus();

  return (
    <BrowserRouter>
      <div className="flex h-screen overflow-hidden bg-[var(--color-bg-primary)]">
        <Sidebar />
        <div className="flex flex-col flex-1 min-w-0">
          <TopBar connectionStatus={connectionStatus} />
          <main className="flex-1 overflow-y-auto">
            <Routes>
              <Route path="/" element={<Navigate to="/devices" replace />} />
              <Route path="/devices" element={<DevicesPage />} />
              <Route path="/audit" element={<AuditPage />} />
              <Route path="/broadcast" element={<BroadcastPage />} />
              <Route path="/settings" element={<SettingsPage />} />
            </Routes>
          </main>
        </div>
      </div>
    </BrowserRouter>
  );
}
