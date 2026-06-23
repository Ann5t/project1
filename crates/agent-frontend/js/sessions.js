// ── Sessions Page ──
import { api, showToast, formatDate } from './api.js';
import { route, navigate } from './app.js';
import { injectSessionSearch } from './search.js';
import { t } from './i18n.js';

route('/sessions', sessionsPage);

// ── Skeleton for session list ──
function renderSessionListSkeleton() {
  return Array(5).fill(0).map(() => `
    <div class="skeleton-list-item">
      <div class="skeleton skeleton-avatar" style="width:36px;height:36px;border-radius:var(--radius-sm);"></div>
      <div style="flex:1;">
        <div class="skeleton skeleton-text medium" style="margin-bottom:6px;"></div>
        <div class="skeleton skeleton-text short"></div>
      </div>
    </div>
  `).join('');
}

// ── Skeleton for session detail ──
function renderSessionDetailSkeleton() {
  return `
    <div style="padding:24px;">
      <div class="skeleton skeleton-title" style="margin-bottom:8px;"></div>
      <div class="skeleton skeleton-text short" style="margin-bottom:28px;"></div>
      ${Array(3).fill(0).map(() => `
        <div class="skeleton-card" style="padding:14px 18px;margin-bottom:10px;">
          <div style="display:flex;gap:8px;align-items:center;margin-bottom:8px;">
            <div class="skeleton" style="width:50px;height:18px;border-radius:20px;"></div>
            <div class="skeleton skeleton-text short" style="margin-bottom:0;"></div>
          </div>
          <div class="skeleton skeleton-text long"></div>
          <div class="skeleton skeleton-text medium"></div>
        </div>
      `).join('')}
    </div>`;
}

// ── Empty state SVG ──
function emptyStateSvg(icon) {
  const svgs = {
    chat: `<svg viewBox="0 0 64 64" fill="none" style="width:64px;height:64px;"><rect x="8" y="10" width="48" height="40" rx="8" stroke="var(--text-tertiary)" stroke-width="2" fill="none"/><path d="M14 22h18M14 30h28M14 38h16" stroke="var(--text-tertiary)" stroke-width="2" stroke-linecap="round"/><circle cx="48" cy="42" r="10" fill="var(--accent-primary)" opacity="0.8"/><path d="M45 42l2 2 4-4" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>`,
    empty: `<svg viewBox="0 0 64 64" fill="none" style="width:64px;height:64px;"><rect x="12" y="8" width="40" height="48" rx="6" stroke="var(--text-tertiary)" stroke-width="2" fill="none"/><path d="M24 22h16M24 32h16M24 42h8" stroke="var(--text-tertiary)" stroke-width="2" stroke-linecap="round"/></svg>`,
  };
  return svgs[icon] || svgs.empty;
}

async function sessionsPage() {
  const container = document.createElement('div');
  container.className = 'page-two-col';

  // Sidebar: session list
  const sidebar = document.createElement('div');
  sidebar.className = 'page-sidebar';
  sidebar.innerHTML = `
    <div style="padding:16px;border-bottom:1px solid var(--border-color);display:flex;justify-content:space-between;align-items:center;">
      <h2 style="font-size:1rem;font-weight:600;">${t('sessions.title')}</h2>
      <button class="btn btn-primary btn-sm" id="new-session-btn">${t('sessions.new_session')}</button>
    </div>
    <div class="session-list" id="session-list">
      ${renderSessionListSkeleton()}
    </div>`;

  // Detail area
  const detail = document.createElement('div');
  detail.className = 'page-detail';
  detail.innerHTML = `
    <div class="empty-state">
      <div class="empty-state-icon">${emptyStateSvg('chat')}</div>
      <div class="empty-state-title">${t('sessions.select_session')}</div>
      <div class="empty-state-text">${t('sessions.select_hint')}</div>
    </div>`;

  container.appendChild(sidebar);
  container.appendChild(detail);

  // Inject search bar
  injectSessionSearch(sidebar);

  // Load sessions
  try {
    const sessions = await api.sessions.list();
    const listEl = sidebar.querySelector('#session-list');
    if (sessions.length === 0) {
      listEl.innerHTML = `
        <div class="empty-state" style="padding:40px 16px;">
          <div class="empty-state-icon">${emptyStateSvg('empty')}</div>
          <div class="empty-state-title">${t('sessions.no_sessions')}</div>
          <div class="empty-state-text">${t('sessions.no_sessions_hint')}</div>
        </div>`;
    } else {
      listEl.innerHTML = sessions.map(s => `
        <div class="session-item" data-id="${s.id}" role="button" tabindex="0">
          <div>
            <div class="session-item-name">${escapeHtml(s.name)}</div>
            <div class="session-item-meta">${escapeHtml(s.model || '')} · ${formatDate(s.updated_at)}</div>
          </div>
          <button class="btn btn-ghost btn-sm session-delete-btn" data-id="${s.id}" title="${t('common.delete')}" aria-label="${t('common.delete')}">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"/><line x1="10" y1="11" x2="10" y2="17"/><line x1="14" y1="11" x2="14" y2="17"/>
            </svg>
          </button>
        </div>`).join('');

      // Click to view session detail
      listEl.querySelectorAll('.session-item').forEach(item => {
        item.addEventListener('click', async (e) => {
          if (e.target.closest('.session-delete-btn')) return;
          const id = item.dataset.id;
          listEl.querySelectorAll('.session-item').forEach(i => i.classList.remove('active'));
          item.classList.add('active');
          // Show skeleton while loading detail
          detail.innerHTML = renderSessionDetailSkeleton();
          try {
            const s = await api.sessions.get(id);
            const msgs = await api.sessions.messages(id);
            detail.innerHTML = renderSessionDetail(s, msgs);
            attachExportListeners(detail, id);
          } catch (e) {
            detail.innerHTML = `<div class="empty-state"><div class="empty-state-icon">${emptyStateSvg('chat')}</div><div class="empty-state-text">${t('sessions.load_detail_failed')}: ${escapeError(e.message)}</div></div>`;
          }
        });
      });

      // Delete buttons
      listEl.querySelectorAll('.session-delete-btn').forEach(btn => {
        btn.addEventListener('click', async (e) => {
          e.stopPropagation();
          const id = btn.dataset.id;
          if (confirm(t('sessions.delete_confirm'))) {
            try {
              await api.sessions.delete(id);
              showToast(t('sessions.deleted'), 'success');
              sessionsPage().then(page => {
                container.replaceWith(page);
              });
            } catch (e) {
              showToast(t('sessions.delete_failed') + ': ' + e.message, 'error');
            }
          }
        });
      });
    }
  } catch (e) {
    sidebar.querySelector('#session-list').innerHTML = `
      <div class="empty-state"><div class="empty-state-text">${t('sessions.load_failed')}: ${escapeError(e.message)}</div></div>`;
  }

  // New session button
  sidebar.querySelector('#new-session-btn').addEventListener('click', () => {
    const name = prompt(t('sessions.session_name_prompt'), t('sessions.default_session_name'));
    if (!name) return;
    api.sessions.create({ name }).then(s => {
      showToast(t('sessions.created'), 'success');
      navigate(`/sessions/${s.id}`);
    }).catch(e => showToast(t('sessions.create_failed') + ': ' + e.message, 'error'));
  });

  return container;
}

function renderSessionDetail(session, messages) {
  return `
    <div style="padding:24px;">
      <div style="display:flex;justify-content:space-between;align-items:flex-start;margin-bottom:24px;flex-wrap:wrap;gap:12px;">
        <div>
          <h2 style="font-size:1.2rem;font-weight:700;">${escapeHtml(session.name)}</h2>
          <span style="color:var(--text-secondary);font-size:0.85rem;">${escapeHtml(session.model || '')} · ${formatDate(session.created_at)}</span>
        </div>
        <div style="display:flex;gap:6px;align-items:center;flex-wrap:wrap;">
          <a href="#/chat/${session.id}" class="btn btn-primary btn-sm">💬 ${t('sessions.continue_chat')}</a>
          <div class="dropdown" style="position:relative;">
            <button class="btn btn-ghost btn-sm export-dropdown-btn" onclick="event.stopPropagation();this.nextElementSibling.classList.toggle('show')">📥 ${t('sessions.export')}</button>
            <div class="dropdown-menu export-dropdown-menu">
              <button class="dropdown-item export-fmt-btn" data-id="${session.id}" data-format="json">${t('sessions.export_json')}</button>
              <button class="dropdown-item export-fmt-btn" data-id="${session.id}" data-format="markdown">${t('sessions.export_md')}</button>
              <button class="dropdown-item export-fmt-btn" data-id="${session.id}" data-format="html">${t('sessions.export_html')}</button>
            </div>
          </div>
        </div>
      </div>
      <div style="display:flex;flex-direction:column;gap:12px;" class="session-messages">
        ${messages.length === 0
          ? `<div class="empty-state"><div class="empty-state-icon">${emptyStateSvg('chat')}</div><div class="empty-state-text">${t('sessions.no_messages')}</div></div>`
          : messages.map(m => `
            <div class="card" style="padding:12px 16px;">
              <div style="display:flex;gap:8px;align-items:center;margin-bottom:4px;">
                <span class="badge ${m.role === 'user' ? 'badge-info' : m.role === 'assistant' ? 'badge-success' : 'badge-warning'}">${escapeHtml(m.role)}</span>
                <span style="font-size:0.75rem;color:var(--text-tertiary);">${formatDate(m.created_at)}</span>
              </div>
              <div style="font-size:0.9rem;line-height:1.6;white-space:pre-wrap;">${escapeHtml(m.content)}</div>
            </div>`).join('')}
      </div>
    </div>`;
}

function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text || '';
  return div.innerHTML;
}

function escapeError(text) {
  if (!text) return '';
  return text.replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

// Attach export button event listeners after rendering session detail.
function attachExportListeners(detailEl, sessionId) {
  // Toggle export dropdown
  const dropdownBtn = detailEl.querySelector('.export-dropdown-btn');
  const dropdownMenu = detailEl.querySelector('.export-dropdown-menu');
  if (dropdownBtn && dropdownMenu) {
    dropdownBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      // Close any other open dropdowns
      document.querySelectorAll('.export-dropdown-menu.show').forEach(m => {
        if (m !== dropdownMenu) m.classList.remove('show');
      });
      dropdownMenu.classList.toggle('show');
    });
  }

  // Format selection buttons
  detailEl.querySelectorAll('.export-fmt-btn').forEach(btn => {
    btn.addEventListener('click', (e) => {
      e.stopPropagation();
      const format = btn.dataset.format;
      dropdownMenu.classList.remove('show');
      api.export.session(sessionId, format).catch(e => showToast(t('chat.export_failed') + ': ' + e.message, 'error'));
    });
  });

  // Close dropdown when clicking outside
  const closeDropdown = (e) => {
    if (!detailEl.contains(e.target)) {
      if (dropdownMenu) dropdownMenu.classList.remove('show');
    }
  };
  document.addEventListener('click', closeDropdown, { once: true });
  // Clean up and re-attach on next render
  detailEl._cleanupExport = () => document.removeEventListener('click', closeDropdown);
}
