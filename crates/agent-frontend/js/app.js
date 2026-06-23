// ── SPA Router ──
import { api, showToast } from './api.js';
import { initShortcuts } from './shortcuts.js';
import { t, setLanguage, getLanguage, applyStaticTranslations } from './i18n.js';
// Import all page modules to register their routes
import './chat.js';
import './sessions.js';
import './config.js';
import './workflow.js';
import './tasks.js';
import './onboarding.js';

const routes = {};
let currentPage = null;
let pageTransitioning = false;

// Register routes
export function route(pattern, handler) {
  routes[pattern] = handler;
}

// Navigate to a route
export function navigate(hash) {
  window.location.hash = hash;
}

// Highlight active nav item
function setActiveNav(path) {
  document.querySelectorAll('.nav-item').forEach(item => {
    const route = item.dataset.route;
    if (route && path.startsWith(route) && route !== '/') {
      item.classList.add('active');
    } else if (route === '/' && (path === '/' || path === '')) {
      item.classList.add('active');
    } else {
      item.classList.remove('active');
    }
  });
}

// ── Page Transition ──
async function render() {
  const hash = window.location.hash.slice(1) || '/';
  const content = document.getElementById('app-content');
  const wrapper = document.getElementById('page-transition-wrapper');

  // Find matching route
  let handler = null;
  let params = {};

  // Exact match first
  if (routes[hash]) {
    handler = routes[hash];
  } else {
    // Pattern match
    for (const [pattern, h] of Object.entries(routes)) {
      const regex = new RegExp('^' + pattern.replace(/:\w+/g, '([^/]+)') + '$');
      const match = hash.match(regex);
      if (match) {
        handler = h;
        const keys = (pattern.match(/:\w+/g) || []).map(k => k.slice(1));
        match.slice(1).forEach((v, i) => { params[keys[i]] = v; });
        break;
      }
    }
  }

  setActiveNav(hash);
  currentPage = hash;

  if (handler) {
    try {
      // Show loader
      wrapper.innerHTML = `<div class="page-loading"><div class="spinner"></div>${t('common.loading')}</div>`;

      const result = await handler(params);

      let newContent;
      if (typeof result === 'string') {
        newContent = document.createElement('div');
        newContent.innerHTML = result;
      } else if (result instanceof HTMLElement) {
        newContent = result;
      } else {
        newContent = document.createElement('div');
        newContent.innerHTML = `<div class="page-loading">${t('page.loaded')}</div>`;
      }

      // Fade transition: fade out old, swap, fade in new
      wrapper.style.opacity = '0';
      wrapper.style.transform = 'translateY(6px)';

      await new Promise(r => setTimeout(r, 120));

      wrapper.innerHTML = '';
      wrapper.appendChild(newContent);
      wrapper.style.opacity = '1';
      wrapper.style.transform = 'translateY(0)';
      pageTransitioning = false;

    } catch (e) {
      wrapper.innerHTML = `
        <div class="page">
          <div class="empty-state">
            <div class="empty-state-icon">
              <svg viewBox="0 0 64 64" fill="none" style="width:64px;height:64px;"><circle cx="32" cy="32" r="30" stroke="var(--error)" stroke-width="2" stroke-dasharray="4 4"/><path d="M32 18v18M32 44v2" stroke="var(--error)" stroke-width="3" stroke-linecap="round"/></svg>
            </div>
            <div class="empty-state-title">${t('page.load_failed')}</div>
            <div class="empty-state-text">${escapeError(e.message)}</div>
            <button class="btn btn-primary" onclick="location.reload()">${t('common.retry')}</button>
          </div>
        </div>`;
      wrapper.style.opacity = '1';
      wrapper.style.transform = 'translateY(0)';
      pageTransitioning = false;
      showToast(e.message, 'error');
    }
  } else {
    wrapper.innerHTML = `
      <div class="page">
        <div class="empty-state">
          <div class="empty-state-icon">
            <svg viewBox="0 0 64 64" fill="none" style="width:64px;height:64px;"><circle cx="32" cy="32" r="30" stroke="var(--text-tertiary)" stroke-width="2"/><path d="M24 24l16 16M40 24l-16 16" stroke="var(--text-tertiary)" stroke-width="3" stroke-linecap="round"/></svg>
          </div>
          <div class="empty-state-title">${t('page.not_found')}</div>
          <div class="empty-state-text">${t('page.not_found_text', { path: escapeError(hash) })}</div>
          <a href="#/" class="btn btn-primary">${t('page.home')}</a>
        </div>
      </div>`;
    wrapper.style.opacity = '1';
    wrapper.style.transform = 'translateY(0)';
    pageTransitioning = false;
  }
}

function escapeError(text) {
  if (!text) return '';
  return text.replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

// Hash change listener
window.addEventListener('hashchange', render);

// Initial render
window.addEventListener('DOMContentLoaded', () => {
  // Style the transition wrapper
  const wrapper = document.getElementById('page-transition-wrapper');
  if (wrapper) {
    wrapper.style.transition = 'opacity 200ms ease-out, transform 200ms ease-out';
    wrapper.style.opacity = '1';
    wrapper.style.transform = 'translateY(0)';
    wrapper.style.minHeight = '100vh';
  }

  // Check onboarding status and initialize language
  api.config.getAll().then(config => {
    if (config.language && (config.language === 'zh' || config.language === 'en')) {
      setLanguage(config.language);
    }
    if (config.onboarding_completed !== 'true') {
      window.location.hash = '#/onboarding';
    }
  }).catch(() => {});
  render();

  initShortcuts();

  // ── Hamburger menu (mobile) ──
  const sidebar = document.getElementById('sidebar');
  const hamburgerBtn = document.getElementById('hamburger-btn');
  const overlay = document.getElementById('sidebar-overlay');
  if (hamburgerBtn && sidebar && overlay) {
    const openSidebar = () => {
      sidebar.classList.add('open');
      overlay.classList.add('active');
    };
    const closeSidebar = () => {
      sidebar.classList.remove('open');
      overlay.classList.remove('active');
    };
    hamburgerBtn.addEventListener('click', openSidebar);
    overlay.addEventListener('click', closeSidebar);
    // Close sidebar when a nav link is clicked on mobile
    sidebar.querySelectorAll('.nav-item').forEach(link => {
      link.addEventListener('click', () => {
        if (window.innerWidth <= 768) closeSidebar();
      });
    });
  }

  // ── Theme toggle ──
  const themeBtn = document.getElementById('theme-toggle-btn');
  const sunIcon = document.getElementById('theme-icon-sun');
  const moonIcon = document.getElementById('theme-icon-moon');
  const themeLabel = document.getElementById('theme-label');
  if (themeBtn) {
    // Load saved theme
    const savedTheme = localStorage.getItem('ai-agent-theme') || 'dark';
    const applyTheme = (theme) => {
      if (theme === 'light') {
        document.body.classList.add('light');
        document.body.classList.remove('dark');
        if (sunIcon) sunIcon.style.display = 'none';
        if (moonIcon) moonIcon.style.display = '';
        if (themeLabel) themeLabel.textContent = t('theme.light');
        document.querySelector('meta[name="theme-color"]')?.setAttribute('content', '#f8f8fc');
      } else {
        document.body.classList.add('dark');
        document.body.classList.remove('light');
        if (sunIcon) sunIcon.style.display = '';
        if (moonIcon) moonIcon.style.display = 'none';
        if (themeLabel) themeLabel.textContent = t('theme.dark');
        document.querySelector('meta[name="theme-color"]')?.setAttribute('content', '#0a0a0f');
      }
      localStorage.setItem('ai-agent-theme', theme);
    };
    applyTheme(savedTheme);
    themeBtn.addEventListener('click', () => {
      const isDark = document.body.classList.contains('dark');
      applyTheme(isDark ? 'light' : 'dark');
    });
  }

  // ── New Chat button ──
  const newChatBtn = document.getElementById('new-chat-btn');
  if (newChatBtn) {
    newChatBtn.addEventListener('click', () => {
      window.location.hash = '#/';
    });
  }

  // ── Keyboard shortcut hint in chat input placeholder ──
  // (handled per-page; we also add a global hint footer on desktop)
  addShortcutHint();
});

function addShortcutHint() {
  // Add a small "Press ? for shortcuts" hint at the bottom of the sidebar
  const sidebar = document.getElementById('sidebar');
  if (!sidebar) return;
  const existing = sidebar.querySelector('.shortcut-hint');
  if (existing) existing.remove();
  const hint = document.createElement('div');
  hint.className = 'shortcut-hint';
  hint.innerHTML = `<kbd>?</kbd> <span>${t('page.shortcut_hint')}</span>`;
  hint.style.cssText = 'padding:6px 12px;font-size:0.72rem;color:var(--text-tertiary);text-align:center;border-top:1px solid var(--border-color);cursor:pointer;display:flex;align-items:center;justify-content:center;gap:6px;transition:color var(--transition-fast);';
  hint.addEventListener('click', () => {
    const overlay = document.getElementById('shortcuts-overlay');
    if (overlay) {
      overlay.style.display = 'flex';
      const modal = document.getElementById('shortcuts-modal');
      if (modal) {
        modal.classList.remove('scale-in');
        modal.offsetHeight;
        modal.classList.add('scale-in');
      }
    }
  });
  hint.addEventListener('mouseenter', () => { hint.style.color = 'var(--text-secondary)'; });
  hint.addEventListener('mouseleave', () => { hint.style.color = 'var(--text-tertiary)'; });
  const footer = sidebar.querySelector('.sidebar-footer');
  if (footer) {
    footer.appendChild(hint);
  }
}

export { render };
