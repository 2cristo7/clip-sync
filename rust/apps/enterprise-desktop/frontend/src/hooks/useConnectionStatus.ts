import { useState, useEffect } from "react";

export type ConnectionStatus = "connected" | "disconnected" | "connecting";

export function useConnectionStatus(): ConnectionStatus {
  const [status, setStatus] = useState<ConnectionStatus>("disconnected");

  useEffect(() => {
    // Stub: In production, this will establish a WebSocket connection
    // to the enterprise server and track connection state.
    // For now, simulate a brief connecting phase then disconnected.
    setStatus("connecting");
    const timer = setTimeout(() => {
      setStatus("disconnected");
    }, 1500);
    return () => clearTimeout(timer);
  }, []);

  return status;
}
