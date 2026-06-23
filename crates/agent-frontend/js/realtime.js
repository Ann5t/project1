// ── Real-time WebSocket Client ──
import { getAuthToken } from './api.js';

// ── State ──
let socket = null;
let reconnectTimer = null;
let reconnectAttempt = 0;
const MAX_RECONNECT_DELAY = 30000; // 30 seconds max
const BASE_RECONNECT_DELAY = 1000; // start at 1 second

// ── Connection indicator (green dot in the sidebar header) ──
function getIndicator() {
  let el = document.querySelector('.ws-indicator');
  if (!el) {
    const header = document.querySelector('.sidebar-header-top');
    if (!header) return null;
    el = document.createElement('div');
    el.className = 'ws-indicator';
    el.title = 'WebSocket disconnected';
    el.style.cssText = 'display:flex;align-items:center;gap:4px;font-size:0.7rem;color:var(--text-tertiary);margin-top:4px;';
    header.appendChild(el);
  }
  return el;
}

function setConnectionStatus(connected) {
  const indicator = getIndicator();
  if (!indicator) return;
  if (connected) {
    indicator.innerHTML = '<span style="display:inline-block;width:7px;height:7px;border-radius:50%;background:#4ade80;box-shadow:0 0 6px #4ade80;"></span> 实时连接';
    indicator.title = 'WebSocket connected';
  } else {
    indicator.innerHTML = '<span style="display:inline-block;width:7px;height:7px;border-radius:50%;background:#f87171;"></span> 离线';
    indicator.title = 'WebSocket disconnected';
  }
}

// ── Toast for workflow completions ──
function showRealtimeToast(message, type) {
  let container = document.querySelector('.toast-container');
  if (!container) {
    container = document.createElement('div');
    container.className = 'toast-container';
    document.body.appendChild(container);
  }
  const toast = document.createElement('div');
  toast.className = `toast toast-${type}`;
  toast.textContent = message;
  container.appendChild(toast);
  setTimeout(() => { toast.remove(); if (!container.children.length) container.remove(); }, 5000);
}

// ── Event handler map ──
// Each handler receives the parsed JSON data from the event.
const eventHandlers = {
  connected(event) {
    console.log('[WS] Connected to server. Active connections:', event.data?.active_connections);
  },

  session_created(event) {
    console.log('[WS] Session created:', event.data?.name);
  },

  message_received(event) {
    console.log('[WS] Message received in session:', event.data?.session_id);
    // Refresh session list if on the sessions page
    refreshSessionList();
  },

  message_sent(event) {
    console.log('[WS] Message sent in session:', event.data?.session_id);
    // Refresh session list
    refreshSessionList();
  },

  workflow_started(event) {
    console.log('[WS] Workflow started:', event.data?.name);
    showRealtimeToast(`工作流 "${event.data?.name}" 已开始运行`, 'info');
  },

  workflow_completed(event) {
    const name = event.data?.name || 'unknown';
    const status = event.data?.status || 'unknown';
    console.log('[WS] Workflow completed:', name, status);
    const type = status === 'success' ? 'success' : 'error';
    showRealtimeToast(`工作流 "${name}" 已完成 (${status})`, type);
  },

  task_executed(event) {
    console.log('[WS] Task executed:', event.data?.name);
    showRealtimeToast(`定时任务 "${event.data?.name}" 已执行`, 'info');
  },

  channel_message_received(event) {
    const channel = event.data?.channel || 'unknown';
    console.log('[WS] Channel message from:', channel);
  },
};

// ── Refresh session list if visible ──
function refreshSessionList() {
  const listEl = document.querySelector('.session-list');
  if (!listEl) return;
  // Only refresh if the sessions page is active
  import('./api.js').then(({ api }) => {
    api.sessions.list().then(sessions => {
      if (sessions.length > 0) {
        const items = listEl.querySelectorAll('.session-item');
        // Simple update: re-render only if count changed
        const currentCount = items.length;
        if (currentCount !== sessions.length) {
          // Trigger a soft re-render by navigating to same hash
          const hash = window.location.hash;
          if (hash === '#/sessions' || hash.startsWith('#/sessions')) {
            // The sessions page will pick up the new sessions on next render
            // For now, just log
            console.log('[WS] Session count changed:', currentCount, '->', sessions.length);
          }
        }
      }
    }).catch(() => {});
  });
}

// ── Connect ──
export function connectWebSocket() {
  if (socket && (socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING)) {
    return; // already connected or connecting
  }

  // Build WebSocket URL
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  const host = window.location.host;
  let wsUrl = `${protocol}//${host}/api/ws`;

  // Append auth token if available
  const token = getAuthToken();
  if (token) {
    wsUrl += `?token=${encodeURIComponent(token)}`;
  }

  console.log('[WS] Connecting to', wsUrl);
  setConnectionStatus(false);

  try {
    socket = new WebSocket(wsUrl);
  } catch (e) {
    console.error('[WS] Failed to create WebSocket:', e);
    scheduleReconnect();
    return;
  }

  socket.onopen = () => {
    console.log('[WS] Connection established');
    setConnectionStatus(true);
    reconnectAttempt = 0; // reset backoff
    if (reconnectTimer) {
      clearTimeout(reconnectTimer);
      reconnectTimer = null;
    }
  };

  socket.onmessage = (msg) => {
    try {
      const event = JSON.parse(msg.data);
      const handler = eventHandlers[event.type];
      if (handler) {
        handler(event);
      } else {
        console.log('[WS] Unhandled event type:', event.type);
      }
    } catch (e) {
      console.error('[WS] Failed to parse message:', e);
    }
  };

  socket.onclose = (e) => {
    console.log('[WS] Connection closed (code:', e.code, 'reason:', e.reason, ')');
    setConnectionStatus(false);
    socket = null;
    scheduleReconnect();
  };

  socket.onerror = (e) => {
    console.error('[WS] Connection error');
    // onclose will fire after onerror, triggering reconnect
  };
}

// ── Disconnect ──
export function disconnectWebSocket() {
  if (reconnectTimer) {
    clearTimeout(reconnectTimer);
    reconnectTimer = null;
  }
  if (socket) {
    socket.onclose = null; // prevent reconnect on manual close
    socket.close();
    socket = null;
  }
  setConnectionStatus(false);
}

// ── Reconnect with exponential backoff ──
function scheduleReconnect() {
  if (reconnectTimer) return; // already scheduled

  reconnectAttempt++;
  const delay = Math.min(
    BASE_RECONNECT_DELAY * Math.pow(2, reconnectAttempt - 1),
    MAX_RECONNECT_DELAY
  );
  // Add jitter: +/- 25%
  const jitter = delay * 0.25 * (Math.random() * 2 - 1);
  const actualDelay = Math.round(delay + jitter);

  console.log(`[WS] Reconnecting in ${actualDelay}ms (attempt ${reconnectAttempt})`);
  setConnectionStatus(false);

  reconnectTimer = setTimeout(() => {
    reconnectTimer = null;
    connectWebSocket();
  }, actualDelay);
}

// ── Auto-connect on page load ──
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', () => {
    // Small delay to let the page render first
    setTimeout(connectWebSocket, 2000);
  });
} else {
  setTimeout(connectWebSocket, 2000);
}

// ── Clean up on page unload ──
window.addEventListener('beforeunload', () => {
  disconnectWebSocket();
});
