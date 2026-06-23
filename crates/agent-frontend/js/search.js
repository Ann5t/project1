// ── Search Functionality ──
import { api, showToast, formatDate } from './api.js';
import { navigate } from './app.js';
import { t } from './i18n.js';

let searchDebounceTimer = null;

/**
 * Add a search bar to the sessions page sidebar.
 * Call this after the sessions page is rendered.
 */
export function injectSessionSearch(sidebarElement) {
  // Remove any existing search bar
  const existing = sidebarElement.querySelector('.search-bar-container');
  if (existing) existing.remove();

  const searchContainer = document.createElement('div');
  searchContainer.className = 'search-bar-container';
  searchContainer.innerHTML = `
    <div class="search-input-wrapper">
      <svg class="search-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="16" height="16">
        <circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/>
      </svg>
      <input type="text" class="search-input" id="session-search-input" placeholder="${t('search.placeholder')}" aria-label="${t('search.placeholder')}">
      <button class="search-clear-btn" id="search-clear-btn" style="display:none;" aria-label="${t('search.clear')}">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14">
          <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
        </svg>
      </button>
    </div>
  `;

  // Insert after the header (title bar) or at the top
  const header = sidebarElement.querySelector('div[style*="padding:16px;border-bottom"]');
  if (header) {
    header.after(searchContainer);
  } else {
    sidebarElement.insertBefore(searchContainer, sidebarElement.firstChild);
  }

  const input = searchContainer.querySelector('#session-search-input');
  const clearBtn = searchContainer.querySelector('#search-clear-btn');

  // Debounced search on input
  input.addEventListener('input', () => {
    const val = input.value.trim();
    clearBtn.style.display = val ? '' : 'none';

    if (searchDebounceTimer) clearTimeout(searchDebounceTimer);
    if (val.length < 1) {
      hideSearchResults(sidebarElement);
      return;
    }

    searchDebounceTimer = setTimeout(() => {
      performSearch(val, sidebarElement);
    }, 300);
  });

  // Clear button
  clearBtn.addEventListener('click', () => {
    input.value = '';
    clearBtn.style.display = 'none';
    hideSearchResults(sidebarElement);
    input.focus();
  });

  // Keyboard shortcut: Ctrl+K in the sessions page focuses search
  const pageDetail = document.getElementById('app-content');
  if (pageDetail) {
    const handler = (e) => {
      if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
        e.preventDefault();
        input.focus();
        input.select();
      }
      if (e.key === 'Escape' && document.activeElement === input) {
        input.blur();
      }
    };
    pageDetail.addEventListener('keydown', handler);
    // Store reference for cleanup
    searchContainer._keydownHandler = handler;
    searchContainer._pageDetail = pageDetail;
  }
}

async function performSearch(query, sidebarElement) {
  try {
    const data = await api.search({ q: query, limit: 20 });

    if (data.results && data.results.length > 0) {
      showSearchResults(data.results, data.total, query, sidebarElement);
    } else {
      showNoResults(query, sidebarElement);
    }
  } catch (e) {
    showToast(t('search.failed') + ': ' + e.message, 'error');
  }
}

function showSearchResults(results, total, query, sidebarElement) {
  let container = sidebarElement.querySelector('.search-results-dropdown');
  if (!container) {
    container = document.createElement('div');
    container.className = 'search-results-dropdown';
    sidebarElement.appendChild(container);
  }

  container.innerHTML = `
    <div class="search-results-header">
      <span>${t('search.results', { total: total, query: escapeHtml(query) })}</span>
      <button class="search-results-close-btn" aria-label="${t('shortcuts.close')}">&times;</button>
    </div>
    <div class="search-results-list">
      ${results.map(r => renderSearchResultItem(r, query)).join('')}
    </div>
  `;

  // Close button
  container.querySelector('.search-results-close-btn').addEventListener('click', () => {
    hideSearchResults(sidebarElement);
  });

  // Click to navigate
  container.querySelectorAll('.search-result-item').forEach(item => {
    item.addEventListener('click', () => {
      const sessionId = item.dataset.sessionId;
      const msgId = item.dataset.msgId;
      if (sessionId) {
        navigate(`/sessions/${sessionId}`);
        hideSearchResults(sidebarElement);
      }
    });
  });
}

function renderSearchResultItem(result, query) {
  const typeLabel = result.type === 'session' ? t('search.type_session') : t('search.type_message');
  const typeClass = result.type === 'session' ? 'badge-info' : 'badge-warning';
  const snippet = highlightMatch(result.snippet, query);
  const scorePct = result.score + '%';

  return `
    <div class="search-result-item" data-session-id="${escapeAttr(result.session_id)}" data-msg-id="${escapeAttr(result.message_id || '')}">
      <div class="search-result-header">
        <span class="search-result-name">${escapeHtml(result.session_name)}</span>
        <span class="badge ${typeClass}" style="font-size:0.7rem;">${typeLabel}</span>
      </div>
      <div class="search-result-snippet">${snippet}</div>
      <div class="search-result-footer">
        <span class="search-result-score">${t('search.relevance')}: ${scorePct}</span>
      </div>
    </div>
  `;
}

function highlightMatch(text, query) {
  if (!text || !query) return escapeHtml(text || '');
  const escaped = escapeHtml(text);
  // Case-insensitive highlight
  const escapedQuery = query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const regex = new RegExp(`(${escapedQuery})`, 'gi');
  return escaped.replace(regex, '<mark class="search-highlight">$1</mark>');
}

function showNoResults(query, sidebarElement) {
  let container = sidebarElement.querySelector('.search-results-dropdown');
  if (!container) {
    container = document.createElement('div');
    container.className = 'search-results-dropdown';
    sidebarElement.appendChild(container);
  }

  container.innerHTML = `
    <div class="search-results-header">
      <span>${t('search.no_results', { query: escapeHtml(query) })}</span>
      <button class="search-results-close-btn" aria-label="${t('shortcuts.close')}">&times;</button>
    </div>
    <div class="search-results-empty">
      <div class="search-results-empty-icon">
        <svg viewBox="0 0 64 64" fill="none" style="width:48px;height:48px;">
          <circle cx="32" cy="32" r="28" stroke="var(--text-tertiary)" stroke-width="2"/>
          <path d="M24 24l16 16M40 24l-16 16" stroke="var(--text-tertiary)" stroke-width="2.5" stroke-linecap="round"/>
        </svg>
      </div>
      <span>${t('search.try_other')}</span>
    </div>
  `;

  container.querySelector('.search-results-close-btn').addEventListener('click', () => {
    hideSearchResults(sidebarElement);
  });
}

function hideSearchResults(sidebarElement) {
  const container = sidebarElement.querySelector('.search-results-dropdown');
  if (container) {
    container.remove();
  }
  const input = sidebarElement.querySelector('#session-search-input');
  if (input) {
    input.value = '';
    const clearBtn = sidebarElement.querySelector('#search-clear-btn');
    if (clearBtn) clearBtn.style.display = 'none';
  }
}

function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text || '';
  return div.innerHTML;
}

function escapeAttr(text) {
  if (!text) return '';
  return text.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/'/g, '&#39;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}
