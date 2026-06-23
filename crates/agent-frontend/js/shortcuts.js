// ── Keyboard Shortcuts & Command Palette ──
// Linear-style command palette with fuzzy search, glass-morphism design.
// Handles all global keyboard shortcuts for the AI Agent frontend.
import { api } from './api.js';
import { t } from './i18n.js';

// ── Navigation History (for Ctrl+[/]) ──
let navHistory = [];
let navIndex = -1;
let isNavigating = false;

function pushNav(hash) {
  if (isNavigating) {
    isNavigating = false;
    return;
  }
  const h = hash || '/';
  if (navHistory[navIndex] !== h) {
    navHistory = navHistory.slice(0, navIndex + 1);
    navHistory.push(h);
    navIndex = navHistory.length - 1;
  }
}

function navigateTo(hash) {
  window.location.hash = hash;
}

// ── Fuzzy Search ──
function fuzzyMatch(query, text) {
  if (!query) return true;
  const q = query.toLowerCase();
  const t = text.toLowerCase();
  let qi = 0;
  for (let ti = 0; ti < t.length && qi < q.length; ti++) {
    if (q[qi] === t[ti]) qi++;
  }
  return qi === q.length;
}

// ═══════════════════════════════════════════
// ── Command Palette (Ctrl/Cmd + K) ──
// ═══════════════════════════════════════════

const CMD_ICONS = {
  chat: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>`,
  workflow: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M4 7h16M4 12h10M4 17h6"/><circle cx="18" cy="7" r="2"/><circle cx="14" cy="12" r="2"/><circle cx="10" cy="17" r="2"/><line x1="16" y1="8.5" x2="13.5" y2="10.5"/><line x1="12.5" y1="13.5" x2="11" y2="15.5"/></svg>`,
  task: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>`,
  theme: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="5"/><line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/><line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/></svg>`,
  sessions: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M23 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/></svg>`,
  config: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="3"/><path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42"/></svg>`,
  plus: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>`,
  search: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.35-4.35"/></svg>`,
};

const BUILT_IN_COMMANDS = [
  {
    id: 'new-chat', title: t('palette.cmd_new_chat'), subtitle: t('palette.cmd_new_chat_sub'),
    icon: 'plus', group: 'actions',
    action: () => navigateTo('#/'),
    keywords: ['chat', 'new', '对话', '新建'],
  },
  {
    id: 'new-workflow', title: t('palette.cmd_new_workflow'), subtitle: t('palette.cmd_new_workflow_sub'),
    icon: 'workflow', group: 'actions',
    action: () => navigateTo('#/workflows'),
    keywords: ['workflow', 'new', '工作流', '自动化', '编排'],
  },
  {
    id: 'new-task', title: t('palette.cmd_new_task'), subtitle: t('palette.cmd_new_task_sub'),
    icon: 'task', group: 'actions',
    action: () => navigateTo('#/tasks'),
    keywords: ['task', 'cron', 'new', '定时', '任务', '计划'],
  },
  {
    id: 'toggle-theme', title: t('palette.cmd_toggle_theme'), subtitle: t('palette.cmd_toggle_theme_sub'),
    icon: 'theme', group: 'actions',
    action: () => {
      const btn = document.getElementById('theme-toggle-btn');
      if (btn) btn.click();
    },
    keywords: ['theme', 'dark', 'light', '主题', '深色', '亮色', '颜色', '模式'],
  },
  {
    id: 'sessions-page', title: t('palette.cmd_sessions'), subtitle: t('palette.cmd_sessions_sub'),
    icon: 'sessions', group: 'nav',
    action: () => navigateTo('#/sessions'),
    keywords: ['sessions', '会话', '管理', '列表'],
  },
  {
    id: 'config-page', title: t('palette.cmd_config'), subtitle: t('palette.cmd_config_sub'),
    icon: 'config', group: 'nav',
    action: () => navigateTo('#/config'),
    keywords: ['config', 'settings', '设置', '配置', '偏好'],
  },
  {
    id: 'shortcuts-ref', title: t('palette.cmd_shortcuts'), subtitle: t('palette.cmd_shortcuts_sub'),
    icon: 'search', group: 'nav',
    action: () => openShortcuts(),
    keywords: ['shortcuts', 'keyboard', '快捷键', '键盘'],
  },
];

let cachedSessions = [];

function highlightMatch(text, query) {
  if (!query || query.length < 2) return text;
  const escaped = query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const regex = new RegExp(`(${escaped})`, 'gi');
  return text.replace(regex, '<mark>$1</mark>');
}

function buildPaletteHTML() {
  return `
    <div class="command-palette-overlay" id="command-palette-overlay" style="display:none;">
      <div class="command-palette" role="dialog" aria-label="命令面板">
        <div class="command-palette-header">
          <span class="command-palette-search-icon">
            ${CMD_ICONS.search}
          </span>
          <input
            type="text"
            class="command-palette-input"
            id="command-palette-input"
            placeholder="${t('palette.placeholder')}"
            autocomplete="off"
            spellcheck="false"
            aria-label="搜索命令"
          >
          <kbd class="command-palette-hint">ESC</kbd>
        </div>
        <div class="command-palette-results" id="command-palette-results"></div>
        <div class="command-palette-footer">
          <span><kbd>↑↓</kbd> ${t('palette.navigate')}</span>
          <span><kbd>Enter</kbd> ${t('palette.select')}</span>
          <span><kbd>Esc</kbd> ${t('palette.close')}</span>
        </div>
      </div>
    </div>`;
}

let paletteItems = [];
let paletteSelected = 0;

function renderPaletteItems(query) {
  const resultsEl = document.getElementById('command-palette-results');
  if (!resultsEl) return;

  const items = [];
  const q = query || '';

  // ── Filter built-in commands ──
  const matchingCommands = q
    ? BUILT_IN_COMMANDS.filter(c =>
        fuzzyMatch(q, c.title + ' ' + (c.subtitle || '') + ' ' + (c.keywords || []).join(' ')))
    : BUILT_IN_COMMANDS;

  // Group by category
  const groupOrder = ['actions', 'nav'];
  const grouped = {};
  matchingCommands.forEach(c => {
    const g = c.group || 'other';
    if (!grouped[g]) grouped[g] = [];
    grouped[g].push(c);
  });

  const groupLabels = { actions: t('palette.section_actions'), nav: t('palette.section_nav'), other: t('palette.section_other') };

  groupOrder.forEach(g => {
    if (grouped[g] && grouped[g].length > 0) {
      if (q || g !== 'actions') {
        items.push({ type: 'header', title: groupLabels[g] || g });
      }
      grouped[g].forEach(c => {
        items.push({ type: 'command', ...c });
      });
    }
  });

  // ── Sessions ──
  if (cachedSessions.length > 0) {
    const matchingSessions = q
      ? cachedSessions.filter(s => fuzzyMatch(q, (s.name || '') + ' ' + (s.id || '') + ' ' + (s.model || '')))
      : cachedSessions.slice(0, 6);

    // Only show recent sessions when not searching
    const sessionsToShow = q ? matchingSessions : cachedSessions.slice(0, 6);

    if (sessionsToShow.length > 0) {
      items.push({ type: 'header', title: q ? t('palette.section_sessions') : t('palette.section_recent') });
      sessionsToShow.forEach(s => {
        items.push({
          type: 'session',
          id: s.id,
          title: s.name || t('palette.unnamed_session'),
          subtitle: (s.model || '') + (s.model ? ' ' : '') + 'ID: ' + (s.id || '').slice(0, 8),
          icon: 'chat',
          action: () => navigateTo('#/chat/' + s.id),
        });
      });
    }
  }

  paletteItems = items.filter(it => it.type !== 'header');
  paletteSelected = 0;

  // Find first actionable item
  for (let i = 0; i < paletteItems.length; i++) {
    if (paletteItems[i].action) {
      paletteSelected = i;
      break;
    }
  }

  if (items.length === 0) {
    resultsEl.innerHTML = `
      <div class="command-palette-empty">
        <div class="command-palette-empty-icon">${CMD_ICONS.search}</div>
        <div>${t('palette.no_results')}</div>
      </div>`;
    return;
  }

  resultsEl.innerHTML = items.map((item, idx) => {
    if (item.type === 'header') {
      return `<div class="command-palette-section-header">${item.title}</div>`;
    }

    const pIdx = paletteItems.indexOf(item);
    const isSelected = pIdx === paletteSelected;
    const useQ = q.length >= 2 ? q : '';

    return `
      <div class="command-palette-item${isSelected ? ' selected' : ''}"
           data-palette-index="${pIdx}"
           role="option"
           aria-selected="${isSelected}">
        <div class="command-palette-item-icon">${CMD_ICONS[item.icon] || CMD_ICONS.chat}</div>
        <div class="command-palette-item-content">
          <div class="command-palette-item-title">${highlightMatch(item.title, useQ)}</div>
          ${item.subtitle ? `<div class="command-palette-item-subtitle">${highlightMatch(item.subtitle, useQ)}</div>` : ''}
        </div>
        <div class="command-palette-item-badge">${item.type === 'command' ? t('palette.badge_command') : ''}</div>
      </div>`;
  }).join('');
}

function updatePaletteSelection() {
  const items = document.querySelectorAll('.command-palette-item');
  items.forEach((el, i) => {
    const pIdx = parseInt(el.dataset.paletteIndex);
    el.classList.toggle('selected', pIdx === paletteSelected);
    el.setAttribute('aria-selected', pIdx === paletteSelected ? 'true' : 'false');
    if (pIdx === paletteSelected) {
      el.scrollIntoView({ block: 'nearest' });
    }
  });
}

function executePaletteAction() {
  if (paletteSelected >= 0 && paletteSelected < paletteItems.length) {
    const item = paletteItems[paletteSelected];
    if (item && item.action) {
      closePalette();
      setTimeout(() => item.action(), 50);
    }
  }
}

function openPalette() {
  let overlay = document.getElementById('command-palette-overlay');
  if (!overlay) {
    document.body.insertAdjacentHTML('beforeend', buildPaletteHTML());
    overlay = document.getElementById('command-palette-overlay');
    if (!overlay) return;
    setupPaletteEvents(overlay);
  }

  // Pre-fetch sessions
  api.sessions.list().then(data => {
    cachedSessions = data.sessions || data || [];
    const input = document.getElementById('command-palette-input');
    if (input && document.getElementById('command-palette-overlay')?.style.display === 'flex') {
      renderPaletteItems(input.value);
    }
  }).catch(() => {
    cachedSessions = [];
  });

  overlay.style.display = 'flex';
  overlay.offsetHeight;
  const palette = overlay.querySelector('.command-palette');
  if (palette) {
    palette.classList.remove('palette-in');
    palette.offsetHeight;
    palette.classList.add('palette-in');
  }

  const input = document.getElementById('command-palette-input');
  if (input) {
    input.value = '';
    setTimeout(() => input.focus(), 60);
  }

  renderPaletteItems('');
}

function closePalette() {
  const overlay = document.getElementById('command-palette-overlay');
  if (!overlay) return;
  const palette = overlay.querySelector('.command-palette');
  if (palette) palette.classList.remove('palette-in');
  overlay.style.opacity = '0';
  setTimeout(() => {
    overlay.style.display = 'none';
    overlay.style.opacity = '';
  }, 150);
}

function isPaletteOpen() {
  const overlay = document.getElementById('command-palette-overlay');
  return overlay && overlay.style.display === 'flex';
}

// ═══════════════════════════════════════════
// ── Shortcuts Modal (?) ──
// ═══════════════════════════════════════════

function openShortcuts() {
  const overlay = document.getElementById('shortcuts-overlay');
  const modal = document.getElementById('shortcuts-modal');
  if (!overlay) return;
  overlay.style.display = 'flex';
  overlay.offsetHeight;
  if (modal) {
    modal.classList.remove('scale-in');
    modal.offsetHeight;
    modal.classList.add('scale-in');
  }
}

function closeShortcuts() {
  const overlay = document.getElementById('shortcuts-overlay');
  if (!overlay) return;
  overlay.style.display = 'none';
}

function isShortcutsOpen() {
  const overlay = document.getElementById('shortcuts-overlay');
  return overlay && overlay.style.display === 'flex';
}

// ═══════════════════════════════════════════
// ── Global Keyboard Listener ──
// ═══════════════════════════════════════════

function isInputFocused() {
  const el = document.activeElement;
  if (!el) return false;
  const tag = el.tagName;
  return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || el.isContentEditable;
}

export function initShortcuts() {
  // ── Setup command palette HTML ──
  if (!document.getElementById('command-palette-overlay')) {
    document.body.insertAdjacentHTML('beforeend', buildPaletteHTML());
    const overlay = document.getElementById('command-palette-overlay');
    if (overlay) setupPaletteEvents(overlay);
  }

  // ── Initialize navigation history ──
  const initialHash = window.location.hash.slice(1) || '/';
  navHistory = [initialHash];
  navIndex = 0;

  window.addEventListener('hashchange', () => {
    const hash = window.location.hash.slice(1) || '/';
    pushNav(hash);
  });

  // ── Global keydown listener ──
  document.addEventListener('keydown', (e) => {
    const paletteOpen = isPaletteOpen();
    const shortcutsOpen = isShortcutsOpen();
    const inInput = isInputFocused();

    // ── Palette is open: palette-specific keys ──
    if (paletteOpen) {
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        closePalette();
        return;
      }
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        e.stopPropagation();
        paletteSelected = Math.min(paletteSelected + 1, paletteItems.length - 1);
        updatePaletteSelection();
        return;
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault();
        e.stopPropagation();
        paletteSelected = Math.max(paletteSelected - 1, 0);
        updatePaletteSelection();
        return;
      }
      if (e.key === 'Enter') {
        e.preventDefault();
        e.stopPropagation();
        executePaletteAction();
        return;
      }
      // Allow typing in input, don't intercept
      return;
    }

    // ── Shortcuts modal is open ──
    if (shortcutsOpen) {
      if (e.key === 'Escape') {
        e.preventDefault();
        closeShortcuts();
        return;
      }
      // Don't intercept other keys when shortcuts modal is open
      return;
    }

    // ── Global: Ctrl/Cmd + K → Command palette (works everywhere) ──
    if ((e.ctrlKey || e.metaKey) && e.key === 'k' && !e.shiftKey && !e.altKey) {
      e.preventDefault();
      openPalette();
      return;
    }

    // ── Global: Escape → close any open panel, or blur input ──
    if (e.key === 'Escape') {
      if (inInput) {
        document.activeElement.blur();
        return;
      }
      return;
    }

    // ── Global: ? or Ctrl/Cmd + / → Show shortcuts ──
    if (
      (e.key === '?' && !e.ctrlKey && !e.metaKey && !e.altKey && !inInput) ||
      ((e.ctrlKey || e.metaKey) && e.key === '/' && !e.shiftKey && !e.altKey)
    ) {
      e.preventDefault();
      openShortcuts();
      return;
    }

    // ── In-input shortcuts (don't capture most global shortcuts when typing) ──
    if (inInput) {
      // Ctrl/Cmd + Enter → Send chat message
      if ((e.ctrlKey || e.metaKey) && e.key === 'Enter' && !e.shiftKey && !e.altKey) {
        const sendBtn = document.getElementById('chat-send-btn');
        if (sendBtn && !sendBtn.disabled) {
          e.preventDefault();
          sendBtn.click();
        }
        return;
      }
      // Let other keys pass through to inputs
      return;
    }

    // ── Non-input shortcuts ──

    // Ctrl/Cmd + Enter → Send chat message (when chat input exists but not focused)
    if ((e.ctrlKey || e.metaKey) && e.key === 'Enter' && !e.shiftKey && !e.altKey) {
      const chatInput = document.getElementById('chat-input');
      if (chatInput && document.activeElement !== chatInput) {
        e.preventDefault();
        chatInput.focus();
      }
      return;
    }

    // Ctrl/Cmd + N → New chat
    if ((e.ctrlKey || e.metaKey) && (e.key === 'n' || e.key === 'N') && !e.shiftKey && !e.altKey) {
      e.preventDefault();
      navigateTo('#/');
      return;
    }

    // Ctrl/Cmd + [ → Previous page
    if ((e.ctrlKey || e.metaKey) && e.key === '[' && !e.shiftKey && !e.altKey) {
      e.preventDefault();
      if (navIndex > 0) {
        navIndex--;
        isNavigating = true;
        navigateTo('#' + navHistory[navIndex]);
      }
      return;
    }

    // Ctrl/Cmd + ] → Next page
    if ((e.ctrlKey || e.metaKey) && e.key === ']' && !e.shiftKey && !e.altKey) {
      e.preventDefault();
      if (navIndex < navHistory.length - 1) {
        navIndex++;
        isNavigating = true;
        navigateTo('#' + navHistory[navIndex]);
      }
      return;
    }

    // Ctrl/Cmd + 1-5 → Tab navigation
    if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey) {
      const navMap = {
        '1': '#/',
        '2': '#/sessions',
        '3': '#/workflows',
        '4': '#/tasks',
        '5': '#/config',
      };
      if (navMap[e.key]) {
        e.preventDefault();
        navigateTo(navMap[e.key]);
        return;
      }
    }
  });

  // ── Shortcuts modal close handlers ──
  const shortcutsOverlay = document.getElementById('shortcuts-overlay');
  const closeBtn = document.getElementById('shortcuts-close-btn');

  if (shortcutsOverlay) {
    shortcutsOverlay.addEventListener('click', (e) => {
      if (e.target === shortcutsOverlay) closeShortcuts();
    });
  }

  if (closeBtn) {
    closeBtn.addEventListener('click', closeShortcuts);
  }
}

// ═══════════════════════════════════════════
// ── Palette Event Setup ──
// ═══════════════════════════════════════════

function setupPaletteEvents(overlay) {
  const input = overlay.querySelector('#command-palette-input');
  const results = overlay.querySelector('#command-palette-results');

  if (input) {
    input.addEventListener('input', () => {
      renderPaletteItems(input.value);
    });

    // Prevent the global keydown from double-handling palette keys
    input.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        closePalette();
        return;
      }
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        e.stopPropagation();
        paletteSelected = Math.min(paletteSelected + 1, paletteItems.length - 1);
        updatePaletteSelection();
        return;
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault();
        e.stopPropagation();
        paletteSelected = Math.max(paletteSelected - 1, 0);
        updatePaletteSelection();
        return;
      }
      if (e.key === 'Enter') {
        e.preventDefault();
        e.stopPropagation();
        executePaletteAction();
        return;
      }
    });
  }

  if (results) {
    results.addEventListener('click', (e) => {
      const item = e.target.closest('.command-palette-item');
      if (!item) return;
      const pIdx = parseInt(item.dataset.paletteIndex);
      if (pIdx >= 0 && pIdx < paletteItems.length) {
        paletteSelected = pIdx;
        executePaletteAction();
      }
    });

    results.addEventListener('mousemove', (e) => {
      const item = e.target.closest('.command-palette-item');
      if (!item) return;
      const pIdx = parseInt(item.dataset.paletteIndex);
      if (pIdx >= 0 && pIdx < paletteItems.length && pIdx !== paletteSelected) {
        paletteSelected = pIdx;
        updatePaletteSelection();
      }
    });
  }

  // Close on overlay click
  overlay.addEventListener('click', (e) => {
    if (e.target === overlay) {
      closePalette();
    }
  });
}
