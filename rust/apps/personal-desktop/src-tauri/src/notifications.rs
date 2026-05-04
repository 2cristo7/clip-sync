//! Desktop notification helper for ClipSync Personal Desktop.
//!
//! Sends native desktop notifications when clipboard content arrives from
//! a remote peer. Implements per-peer throttling (max 1 notification per
//! second per peer) to avoid bombardment.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tracing::{debug, info};

/// Minimum interval between notifications from the same peer.
const THROTTLE_INTERVAL: Duration = Duration::from_secs(1);

/// Notification manager with per-peer throttle state.
pub struct NotificationManager {
    /// Last notification time per peer device ID.
    last_notified: Mutex<HashMap<String, Instant>>,
    /// Whether notifications are enabled.
    enabled: bool,
}

/// Payload type for notification formatting.
#[derive(Debug, Clone)]
pub enum ClipKind {
    Text(String),
    Image,
    File(String),
}

impl NotificationManager {
    /// Create a new notification manager.
    pub fn new(enabled: bool) -> Self {
        Self {
            last_notified: Mutex::new(HashMap::new()),
            enabled,
        }
    }

    /// Attempt to send a notification for a received clipboard event.
    ///
    /// Returns `true` if the notification was sent, `false` if throttled or
    /// disabled.
    pub fn notify(&self, peer_name: &str, peer_id: &str, kind: &ClipKind) -> bool {
        if !self.enabled {
            debug!("notifications disabled, skipping");
            return false;
        }

        if !self.should_notify(peer_id) {
            debug!(peer = %peer_id, "notification throttled");
            return false;
        }

        let (title, body) = format_notification(peer_name, kind);
        self.send_native(&title, &body);
        true
    }

    /// Check throttle and record notification time if allowed.
    ///
    /// Returns `true` if the notification should proceed.
    fn should_notify(&self, peer_id: &str) -> bool {
        let mut map = self.last_notified.lock().unwrap();
        let now = Instant::now();

        if let Some(last) = map.get(peer_id) {
            if now.duration_since(*last) < THROTTLE_INTERVAL {
                return false;
            }
        }

        map.insert(peer_id.to_string(), now);
        true
    }

    /// Send the actual native notification.
    ///
    /// Uses `notify-rust` on Linux and macOS notification center on macOS.
    fn send_native(&self, title: &str, body: &str) {
        info!(title = %title, body = %body, "sending notification");

        #[cfg(target_os = "macos")]
        {
            // Use osascript for simple macOS notifications (no extra deps).
            let script = format!(
                "display notification \"{}\" with title \"{}\"",
                body.replace('"', "\\\""),
                title.replace('"', "\\\""),
            );
            let _ = std::process::Command::new("osascript")
                .arg("-e")
                .arg(&script)
                .spawn();
        }

        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("notify-send")
                .arg(title)
                .arg(body)
                .spawn();
        }

        #[cfg(target_os = "windows")]
        {
            // Windows: use PowerShell toast notification.
            let ps = format!(
                "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null; \
                 $template = [Windows.UI.Notifications.ToastTemplateType]::ToastText02; \
                 $xml = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent($template); \
                 $texts = $xml.GetElementsByTagName('text'); \
                 $texts[0].AppendChild($xml.CreateTextNode('{title}')) | Out-Null; \
                 $texts[1].AppendChild($xml.CreateTextNode('{body}')) | Out-Null; \
                 $toast = [Windows.UI.Notifications.ToastNotification]::new($xml); \
                 [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('ClipSync').Show($toast)",
                title = title.replace('\'', "''"),
                body = body.replace('\'', "''"),
            );
            let _ = std::process::Command::new("powershell")
                .arg("-Command")
                .arg(&ps)
                .spawn();
        }
    }
}

/// Format notification title and body from clip kind.
fn format_notification(peer_name: &str, kind: &ClipKind) -> (String, String) {
    let title = format!("Clipboard from {}", peer_name);
    let body = match kind {
        ClipKind::Text(text) => {
            if text.len() > 80 {
                format!("{}...", &text[..77])
            } else {
                text.clone()
            }
        }
        ClipKind::Image => "Image received".to_string(),
        ClipKind::File(filename) => format!("File: {}", filename),
    };
    (title, body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn throttle_blocks_rapid_notifications() {
        let mgr = NotificationManager::new(false); // disabled so no actual notif

        // First should pass throttle check.
        assert!(mgr.should_notify("peer-a"));
        // Immediately after should be throttled.
        assert!(!mgr.should_notify("peer-a"));
        assert!(!mgr.should_notify("peer-a"));
    }

    #[test]
    fn throttle_allows_after_interval() {
        let mgr = NotificationManager::new(false);

        assert!(mgr.should_notify("peer-b"));
        // Wait just over the throttle interval.
        thread::sleep(Duration::from_millis(1050));
        assert!(mgr.should_notify("peer-b"));
    }

    #[test]
    fn throttle_independent_per_peer() {
        let mgr = NotificationManager::new(false);

        assert!(mgr.should_notify("peer-x"));
        assert!(mgr.should_notify("peer-y"));
        // peer-x is throttled but peer-y just went through
        assert!(!mgr.should_notify("peer-x"));
        assert!(!mgr.should_notify("peer-y"));
    }

    #[test]
    fn notify_returns_false_when_disabled() {
        let mgr = NotificationManager::new(false);
        let kind = ClipKind::Text("hello".to_string());
        assert!(!mgr.notify("MyPhone", "peer-1", &kind));
    }

    #[test]
    fn format_text_short() {
        let (title, body) = format_notification("MyPhone", &ClipKind::Text("hello world".to_string()));
        assert_eq!(title, "Clipboard from MyPhone");
        assert_eq!(body, "hello world");
    }

    #[test]
    fn format_text_truncated() {
        let long = "a".repeat(100);
        let (_, body) = format_notification("Peer", &ClipKind::Text(long));
        assert_eq!(body.len(), 80); // 77 chars + "..."
        assert!(body.ends_with("..."));
    }

    #[test]
    fn format_image() {
        let (_, body) = format_notification("Peer", &ClipKind::Image);
        assert_eq!(body, "Image received");
    }

    #[test]
    fn format_file() {
        let (_, body) = format_notification("Peer", &ClipKind::File("doc.pdf".to_string()));
        assert_eq!(body, "File: doc.pdf");
    }

    #[test]
    fn throttle_bombardment_five_events() {
        let mgr = NotificationManager::new(false);

        let mut passed = 0;
        for _ in 0..5 {
            if mgr.should_notify("bomber") {
                passed += 1;
            }
        }
        // Only the first should pass.
        assert_eq!(passed, 1);
    }
}
