// ── API Client ──
const BASE = '/api';
const TOKEN_KEY = 'ai-agent-auth-token';

// ── Auth token helpers ──
export function getAuthToken() {
  return localStorage.getItem(TOKEN_KEY);
}

export function setAuthToken(token) {
  localStorage.setItem(TOKEN_KEY, token);
}

export function clearAuthToken() {
  localStorage.removeItem(TOKEN_KEY);
}

async function request(url, options = {}) {
  const token = getAuthToken();
  const headers = { 'Content-Type': 'application/json', ...options.headers };
  if (token) {
    headers['Authorization'] = `Bearer ${token}`;
  }
  try {
    const resp = await fetch(BASE + url, {
      headers,
      ...options,
    });
    // Handle 401 — token invalid or expired
    if (resp.status === 401) {
      clearAuthToken();
      window.location.hash = '#/login';
      throw new Error('Unauthorized — please login again');
    }
    const data = await resp.json();
    if (!resp.ok) {
      throw new Error(data.error || `HTTP ${resp.status}`);
    }
    return data;
  } catch (e) {
    if (e.message?.includes('Failed to fetch')) {
      throw new Error('Network error — is the server running?');
    }
    throw e;
  }
}

export const api = {
  // Auth
  auth: {
    status: () => request('/auth/status'),
    login: (token) => request('/auth/login', { method: 'POST', body: JSON.stringify({ token }) }),
  },

  // Health
  health: () => request('/health'),

  // Config
  config: {
    getAll: () => request('/config'),
    updateAll: (cfg) => request('/config', { method: 'PUT', body: JSON.stringify(cfg) }),
    get: (key) => request(`/config/${encodeURIComponent(key)}`),
    set: (key, value) => request(`/config/${encodeURIComponent(key)}`, { method: 'PUT', body: JSON.stringify({ value }) }),
  },

  // Sessions
  sessions: {
    list: () => request('/sessions'),
    create: (data) => request('/sessions', { method: 'POST', body: JSON.stringify(data) }),
    get: (id) => request(`/sessions/${id}`),
    update: (id, data) => request(`/sessions/${id}`, { method: 'PUT', body: JSON.stringify(data) }),
    delete: (id) => request(`/sessions/${id}`, { method: 'DELETE' }),
    messages: (id) => request(`/sessions/${id}/messages`),
  },

  // Chat
  chat: {
    send: (data) => request('/chat', { method: 'POST', body: JSON.stringify(data) }),
  },

  // Channels
  channels: {
    list: () => request('/channels'),
    create: (data) => request('/channels', { method: 'POST', body: JSON.stringify(data) }),
    update: (id, data) => request(`/channels/${id}`, { method: 'PUT', body: JSON.stringify(data) }),
    delete: (id) => request(`/channels/${id}`, { method: 'DELETE' }),
    test: (id) => request(`/channels/${id}/test`, { method: 'POST' }),
  },

  // Workflows
  workflows: {
    list: () => request('/workflows'),
    create: (data) => request('/workflows', { method: 'POST', body: JSON.stringify(data) }),
    get: (id) => request(`/workflows/${id}`),
    update: (id, data) => request(`/workflows/${id}`, { method: 'PUT', body: JSON.stringify(data) }),
    delete: (id) => request(`/workflows/${id}`, { method: 'DELETE' }),
    run: (id) => request(`/workflows/${id}/run`, { method: 'POST' }),
    runs: (id) => request(`/workflows/${id}/runs`),
  },

  // Search
  search: (params) => {
    const qs = new URLSearchParams();
    if (params.q) qs.set('q', params.q);
    if (params.type) qs.set('type', params.type);
    if (params.page) qs.set('page', params.page);
    if (params.limit) qs.set('limit', params.limit);
    return request(`/search?${qs.toString()}`);
  },

  // Tasks
  tasks: {
    list: () => request('/tasks'),
    create: (data) => request('/tasks', { method: 'POST', body: JSON.stringify(data) }),
    get: (id) => request(`/tasks/${id}`),
    update: (id, data) => request(`/tasks/${id}`, { method: 'PUT', body: JSON.stringify(data) }),
    delete: (id) => request(`/tasks/${id}`, { method: 'DELETE' }),
    run: (id) => request(`/tasks/${id}/run`, { method: 'POST' }),
    logs: (id) => request(`/tasks/${id}/logs`),
  },

  // Export
  export: {
    session: (id, format) => downloadBlob(`/export/session/${id}?format=${encodeURIComponent(format)}`, `session-${id}.${extForFormat(format)}`),
    workflowRuns: (id, format) => downloadBlob(`/export/workflow/${id}/runs?format=${encodeURIComponent(format)}`, `workflow-${id}-runs.${extForFormat(format)}`),
    bulk: (sessionIds, format) => {
      const token = getAuthToken();
      const headers = { 'Content-Type': 'application/json' };
      if (token) { headers['Authorization'] = `Bearer ${token}`; }
      return fetch(BASE + '/export/bulk', {
        method: 'POST',
        headers,
        body: JSON.stringify({ session_ids: sessionIds, format }),
      }).then(resp => {
        if (!resp.ok) {
          return resp.json().then(err => { throw new Error(err.error || `HTTP ${resp.status}`); });
        }
        return resp.blob();
      }).then(blob => {
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `sessions-export.zip`;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);
      });
    },
  },
};

// ── Toast notifications ──
export function showToast(message, type = 'info') {
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
  const removeToast = () => {
    toast.classList.add('toast-exit');
    setTimeout(() => {
      toast.remove();
      if (!container.children.length) container.remove();
    }, 200);
  };
  setTimeout(removeToast, 4000);
  toast.addEventListener('click', removeToast);
}

// ── Formatting helpers ──
export function formatDate(dateStr) {
  if (!dateStr) return '';
  try {
    const d = new Date(dateStr);
    const now = new Date();
    const diff = now - d;
    if (diff < 60000) return '刚刚';
    if (diff < 3600000) return `${Math.floor(diff / 60000)}分钟前`;
    if (diff < 86400000) return `${Math.floor(diff / 3600000)}小时前`;
    return d.toLocaleDateString('zh-CN', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
  } catch { return dateStr; }
}

// ── Export helpers ──
function downloadBlob(url, filename) {
  const token = getAuthToken();
  const headers = {};
  if (token) { headers['Authorization'] = `Bearer ${token}`; }
  return fetch(BASE + url, { headers })
    .then(resp => {
      if (!resp.ok) {
        return resp.json().then(err => { throw new Error(err.error || `HTTP ${resp.status}`); });
      }
      const disposition = resp.headers.get('Content-Disposition') || '';
      const match = disposition.match(/filename="?(.+?)"?$/);
      const downloadName = match ? match[1] : filename;
      return resp.blob().then(blob => ({ blob, downloadName }));
    })
    .then(({ blob, downloadName }) => {
      const objectUrl = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = objectUrl;
      a.download = downloadName;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(objectUrl);
    });
}

function extForFormat(format) {
  switch (format) {
    case 'markdown': return 'md';
    case 'html': return 'html';
    case 'csv': return 'csv';
    default: return 'json';
  }
}
