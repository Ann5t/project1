// ── Authentication / Login Page ──
import { api, getAuthToken, setAuthToken, clearAuthToken } from './api.js';
import { route, navigate } from './app.js';

route('/login', loginPage);

async function loginPage() {
  const container = document.createElement('div');
  container.className = 'page';

  // Build login UI
  container.innerHTML = `
    <div class="login-center">
      <div class="login-card scale-in">
        <div class="login-logo">
          <svg viewBox="0 0 48 48" fill="none">
            <defs><linearGradient id="login-grad" x1="0" y1="0" x2="48" y2="48"><stop offset="0%" stop-color="#8b5cf6"/><stop offset="100%" stop-color="#3b82f6"/></linearGradient></defs>
            <rect width="48" height="48" rx="12" fill="url(#login-grad)"/>
            <path d="M15 33V15l9 6 9-6v18" stroke="white" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" fill="none"/>
          </svg>
        </div>
        <h1 class="login-title">AI Agent</h1>
        <p class="login-subtitle">请输入管理员令牌以访问控制台</p>
        <form id="login-form" class="login-form">
          <div class="input-wrapper">
            <input class="input input-lg" type="password" id="login-token-input" placeholder="输入访问令牌..." autocomplete="off" spellcheck="false" aria-label="访问令牌">
          </div>
          <p id="login-error" class="login-error" style="display:none;"></p>
          <button type="submit" class="btn btn-primary btn-full btn-lg" id="login-btn">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:18px;height:18px;"><path d="M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4M10 17l5-5-5-5M13.8 12H3"/></svg>
            登录
          </button>
        </form>
        <p class="login-hint">令牌在服务器首次启动时生成，你可以从服务器日志或设置页面查看。</p>
      </div>
    </div>`;

  // Bind form events after the DOM is inserted
  setTimeout(() => {
    const form = container.querySelector('#login-form');
    const input = container.querySelector('#login-token-input');
    const errorEl = container.querySelector('#login-error');
    const btn = container.querySelector('#login-btn');

    form.addEventListener('submit', async (e) => {
      e.preventDefault();
      const token = input.value.trim();
      if (!token) {
        showLoginError(errorEl, '请输入令牌');
        input.focus();
        return;
      }
      try {
        btn.disabled = true;
        btn.textContent = '验证中...';
        errorEl.style.display = 'none';
        const result = await api.auth.login(token);
        if (result.valid) {
          setAuthToken(token);
          navigate('/');
        }
      } catch (err) {
        showLoginError(errorEl, err.message || '令牌验证失败');
        input.focus();
      } finally {
        btn.disabled = false;
        btn.innerHTML = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:18px;height:18px;"><path d="M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4M10 17l5-5-5-5M13.8 12H3"/></svg>登录`;
      }
    });

    // Focus the input on load
    input.focus();
  }, 100);

  return container;
}

function showLoginError(el, msg) {
  el.textContent = msg;
  el.style.display = 'block';
}

// ── Auth guard: check auth status on initial load ──
window.addEventListener('DOMContentLoaded', () => {
  // Defer slightly so the main app init happens first
  setTimeout(async () => {
    try {
      const status = await api.auth.status();
      if (status.auth_enabled && status.configured) {
        const token = getAuthToken();
        if (!token && window.location.hash !== '#/login') {
          navigate('/login');
        }
      }
    } catch (_) {
      // If the /api/auth/status call fails (server not up yet, etc.), ignore
    }
  }, 300);
});
