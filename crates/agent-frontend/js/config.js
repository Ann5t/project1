// ── Settings Page ──
import { api, showToast } from './api.js';
import { route } from './app.js';
import { t, setLanguage } from './i18n.js';

route('/config', configPage);

// ── Config page skeleton ──
function configSkeleton() {
  return Array(3).fill(0).map(() => `
    <div class="config-section">
      <div class="skeleton" style="width:140px;height:16px;margin-bottom:12px;"></div>
      <div class="skeleton-card">
        <div style="margin-bottom:16px;">
          <div class="skeleton skeleton-text short" style="margin-bottom:6px;"></div>
          <div class="skeleton" style="width:100%;height:40px;margin-bottom:4px;"></div>
          <div class="skeleton skeleton-text short"></div>
        </div>
        <div style="margin-bottom:16px;">
          <div class="skeleton skeleton-text short" style="margin-bottom:6px;"></div>
          <div class="skeleton" style="width:100%;height:40px;"></div>
        </div>
        <div class="skeleton" style="width:120px;height:36px;border-radius:var(--radius-md);"></div>
      </div>
    </div>
  `).join('');
}

async function configPage() {
  const container = document.createElement('div');
  container.className = 'page';

  // Show skeleton until config loads
  container.innerHTML = `
    <div class="page-header">
      <h1 class="page-title">${t('settings.title')}</h1>
      <p class="page-subtitle">${t('settings.subtitle')}</p>
    </div>
    ${configSkeleton()}`;

  try {
    const config = await api.config.getAll();
    container.innerHTML = `
      <div class="page-header">
        <h1 class="page-title">${t('settings.title')}</h1>
        <p class="page-subtitle">${t('settings.subtitle')}</p>
      </div>
      ${renderLLMSection(config)}
      ${renderChannelSection()}
      ${renderUISection(config)}
      ${renderSystemSection(config)}`;
  } catch (e) {
    container.innerHTML = `
      <div class="page-header"><h1 class="page-title">${t('settings.title')}</h1></div>
      <div class="empty-state">
        <div class="empty-state-icon">
          <svg viewBox="0 0 64 64" fill="none" style="width:64px;height:64px;"><circle cx="32" cy="32" r="30" stroke="var(--error)" stroke-width="2" stroke-dasharray="4 4"/><path d="M32 18v18M32 44v2" stroke="var(--error)" stroke-width="3" stroke-linecap="round"/></svg>
        </div>
        <div class="empty-state-text">${t('settings.config_load_failed')}: ${escapeError(e.message)}</div>
        <button class="btn btn-primary" onclick="location.reload()">${t('common.retry')}</button>
      </div>`;
  }

  // Save handler
  setTimeout(() => {
    container.querySelectorAll('.save-config-btn').forEach(btn => {
      btn.addEventListener('click', async () => {
        const section = btn.dataset.section;
        const inputs = container.querySelectorAll(`[data-config-section="${section}"]`);
        const updates = {};
        inputs.forEach(input => {
          updates[input.dataset.key] = input.type === 'checkbox' ? input.checked.toString() : input.value;
        });
        try {
          // Show saving state
          const originalText = btn.textContent;
          btn.disabled = true;
          btn.textContent = t('settings.saving');
          await api.config.updateAll(updates);
          showToast(t('settings.saved'), 'success');
          btn.textContent = originalText;
          btn.disabled = false;
        } catch (e) {
          showToast(t('settings.save_failed') + ': ' + e.message, 'error');
          btn.disabled = false;
        }
      });
    });

    // Channel test
    container.querySelectorAll('.test-channel-btn').forEach(btn => {
      btn.addEventListener('click', async () => {
        const id = btn.dataset.id;
        const originalText = btn.textContent;
        btn.disabled = true;
        btn.textContent = t('channels.testing');
        try {
          await api.channels.test(id);
          showToast(t('channels.test_success'), 'success');
        } catch (e) {
          showToast(t('channels.test_failed') + ': ' + e.message, 'error');
        } finally {
          btn.textContent = originalText;
          btn.disabled = false;
        }
      });
    });

    // Save feishu channel
    container.querySelector('#save-feishu-btn')?.addEventListener('click', async () => {
      const feishuConfig = {
        app_id: container.querySelector('#feishu-app-id')?.value || '',
        app_secret: container.querySelector('#feishu-app-secret')?.value || '',
        verification_token: container.querySelector('#feishu-verification-token')?.value || '',
      };
      const btn = container.querySelector('#save-feishu-btn');
      await saveChannel(btn, 'feishu', t('channels.feishu_label'), feishuConfig);
    });

    // Save QQ Bot channel
    container.querySelector('#save-qq-btn')?.addEventListener('click', async () => {
      const qqConfig = {
        app_id: container.querySelector('#qq-app-id')?.value || '',
        client_secret: container.querySelector('#qq-client-secret')?.value || '',
        bot_secret: container.querySelector('#qq-bot-secret')?.value || '',
      };
      const btn = container.querySelector('#save-qq-btn');
      await saveChannel(btn, 'qq', t('channels.qq_label'), qqConfig);
    });

    // Save WeChat Work channel
    container.querySelector('#save-wx-btn')?.addEventListener('click', async () => {
      const wxConfig = {
        corp_id: container.querySelector('#wx-corp-id')?.value || '',
        corp_secret: container.querySelector('#wx-corp-secret')?.value || '',
        agent_id: container.querySelector('#wx-agent-id')?.value || '',
        token: container.querySelector('#wx-token')?.value || '',
        encoding_aes_key: container.querySelector('#wx-encoding-aes-key')?.value || '',
      };
      const btn = container.querySelector('#save-wx-btn');
      await saveChannel(btn, 'wechat_work', t('channels.wechat_label'), wxConfig);
    });

    // Save Webhook channel
    container.querySelector('#save-webhook-btn')?.addEventListener('click', async () => {
      const webhookPath = container.querySelector('#webhook-path')?.value?.trim() || '';
      if (!webhookPath) {
        showToast(t('channels.enter_path'), 'error');
        return;
      }
      const webhookConfig = {
        webhook_url_path: webhookPath,
        secret: container.querySelector('#webhook-secret')?.value || '',
        json_message_path: container.querySelector('#webhook-json-path')?.value || 'message',
        response_template: container.querySelector('#webhook-response-template')?.value || '{"reply": "{{reply}}"}',
      };
      const btn = container.querySelector('#save-webhook-btn');
      await saveChannel(btn, 'webhook', t('channels.webhook_label') + ' (' + webhookPath + ')', webhookConfig);
      // Update the displayed URL
      updateWebhookUrl();
    });

    // Copy webhook URL
    container.querySelector('#copy-webhook-url-btn')?.addEventListener('click', () => {
      const urlEl = document.getElementById('webhook-full-url');
      if (urlEl && urlEl.textContent && !urlEl.textContent.includes('YOUR_PATH')) {
        navigator.clipboard.writeText(urlEl.textContent).then(() => {
          showToast(t('channels.webhook_copied'), 'success');
        }).catch(() => {
          showToast(t('channels.webhook_copy_failed'), 'error');
        });
      } else {
        showToast(t('channels.webhook_path_required'), 'info');
      }
    });

    // Live update of webhook full URL as user types the path
    container.querySelector('#webhook-path')?.addEventListener('input', updateWebhookUrl);

    function updateWebhookUrl() {
      const pathInput = document.getElementById('webhook-path');
      const urlEl = document.getElementById('webhook-full-url');
      if (pathInput && urlEl) {
        const path = pathInput.value.trim();
        if (path) {
          const base = window.location.origin || 'http://localhost:3000';
          urlEl.textContent = base + '/api/channels/webhook/' + path;
        } else {
          const base = window.location.origin || 'http://localhost:3000';
          urlEl.textContent = base + '/api/channels/webhook/YOUR_PATH';
        }
      }
    }

    async function saveChannel(btn, channelType, channelName, config) {
      const originalText = btn.textContent;
      btn.disabled = true;
      btn.textContent = t('settings.saving');
      try {
        const channels = await api.channels.list();
        const existing = channels.find(c => c.channel_type === channelType);
        if (existing) {
          await api.channels.update(existing.id, { config: config, enabled: true });
        } else {
          await api.channels.create({
            channel_type: channelType,
            name: channelName,
            config: config,
          });
        }
        showToast(channelName + ' ' + t('settings.saved'), 'success');
      } catch (e) {
        showToast(t('settings.save_failed') + ': ' + e.message, 'error');
      } finally {
        btn.textContent = originalText;
        btn.disabled = false;
      }
    }
      // Language switcher: immediately apply language change
    const langSelect = container.querySelector("#config-language");
    if (langSelect) {
      langSelect.addEventListener("change", () => {
        const newLang = langSelect.value;
        if (setLanguage(newLang)) {
          configPage().then(newPage => {
            container.replaceWith(newPage);
          });
        }
      });
    }
    }, 100);

  return container;
}

function renderLLMSection(config) {
  return `
    <div class="config-section">
      <h3 class="config-section-title">🤖 ${t('settings.llm_config')}</h3>
      <div class="card">
        <div class="form-group">
          <label class="form-label">${t('settings.api_key')}</label>
          <input class="input" type="password" data-config-section="llm" data-key="api_key" value="${escapeAttr(config.api_key || '')}" placeholder="sk-..." aria-label="${t('settings.api_key')}">
          <p class="form-help">${t('settings.api_key_hint').replace('platform.deepseek.com', '<a href="https://platform.deepseek.com" target="_blank" rel="noopener">platform.deepseek.com</a>')}</p>
        </div>
        <div class="form-group">
          <label class="form-label">${t('settings.base_url')}</label>
          <input class="input" data-config-section="llm" data-key="base_url" value="${escapeAttr(config.base_url || 'https://api.deepseek.com/v1')}">
        </div>
        <div class="form-group">
          <label class="form-label">${t('settings.flash_model')}</label>
          <input class="input" data-config-section="llm" data-key="flash_model" value="${escapeAttr(config.flash_model || 'deepseek-chat')}" placeholder="deepseek-chat">
        </div>
        <div class="form-group">
          <label class="form-label">${t('settings.pro_model')}</label>
          <input class="input" data-config-section="llm" data-key="pro_model" value="${escapeAttr(config.pro_model || 'deepseek-reasoner')}" placeholder="deepseek-reasoner">
        </div>
        <div class="form-group">
          <label class="form-label">${t('settings.system_prompt')}</label>
          <textarea class="input textarea" data-config-section="llm" data-key="system_prompt" rows="3" aria-label="${t('settings.system_prompt')}">${escapeAttr(config.system_prompt || 'You are a helpful AI assistant.')}</textarea>
        </div>
        <div style="display:flex;gap:16px;">
          <div class="form-group" style="flex:1;">
            <label class="form-label">${t('settings.temperature')}</label>
            <input class="input" type="number" data-config-section="llm" data-key="temperature" value="${config.temperature || '0.7'}" min="0" max="2" step="0.1" aria-label="${t('settings.temperature')}">
          </div>
          <div class="form-group" style="flex:1;">
            <label class="form-label">${t('settings.max_tokens')}</label>
            <input class="input" type="number" data-config-section="llm" data-key="max_tokens" value="${config.max_tokens || '4096'}" aria-label="${t('settings.max_tokens')}">
          </div>
        </div>
        <button class="btn btn-primary save-config-btn" data-section="llm">${t('settings.save_llm')}</button>
      </div>
    </div>`;
}

function renderChannelSection() {
  return `
    <div class="config-section">
      <h3 class="config-section-title">🔗 ${t('settings.channels')}</h3>
      <div class="card">
        <div class="form-group">
          <label class="form-label">📱 ${t('channels.feishu_label')}</label>
          <p class="form-help" style="margin-bottom:12px;">${t('channels.feishu_hint')}</p>
        </div>
        <div class="form-group">
          <label class="form-label">${t('channels.app_id')}</label>
          <input class="input" id="feishu-app-id" placeholder="cli_..." data-config-section="feishu" aria-label="${t('channels.feishu_label')} ${t('channels.app_id')}">
        </div>
        <div class="form-group">
          <label class="form-label">${t('channels.app_secret')}</label>
          <input class="input" type="password" id="feishu-app-secret" placeholder="..." data-config-section="feishu" aria-label="${t('channels.feishu_label')} ${t('channels.app_secret')}">
        </div>
        <div class="form-group">
          <label class="form-label">${t('channels.verification_token')}</label>
          <input class="input" id="feishu-verification-token" placeholder="..." data-config-section="feishu" aria-label="${t('channels.feishu_label')} ${t('channels.verification_token')}">
        </div>
        <div class="form-group">
          <label class="form-label">${t('channels.callback_label')}</label>
          <code style="background:var(--bg-tertiary);padding:6px 12px;border-radius:var(--radius-sm);font-size:0.85rem;">http://你的IP:3000/api/channels/feishu/callback</code>
        </div>
        <div style="display:flex;gap:8px;">
          <button class="btn btn-primary" id="save-feishu-btn">${t('channels.save_feishu')}</button>
          <button class="btn btn-outline test-channel-btn" data-id="feishu" style="margin-left:8px;">${t('channels.test_connection')}</button>
        </div>
      </div>
      <div class="card" style="margin-top:16px;">
        <div class="form-group">
          <label class="form-label">🐧 ${t('channels.qq_label')}</label>
          <p class="form-help" style="margin-bottom:12px;">${t('channels.qq_hint').replace('QQ 开放平台', '<a href="https://q.qq.com/" target="_blank" rel="noopener">QQ 开放平台</a>').replace('QQ Open Platform', '<a href="https://q.qq.com/" target="_blank" rel="noopener">QQ Open Platform</a>')}</p>
        </div>
        <div class="form-group">
          <label class="form-label">Bot App ID</label>
          <input class="input" id="qq-app-id" placeholder="1020xxxxx" data-config-section="qq" aria-label="QQ App ID">
          <p class="form-help">${t('channels.qq_app_id_hint')}</p>
        </div>
        <div class="form-group">
          <label class="form-label">${t('channels.qq_client_secret')}</label>
          <input class="input" type="password" id="qq-client-secret" placeholder="..." data-config-section="qq" aria-label="QQ Client Secret">
          <p class="form-help">${t('channels.qq_client_secret_hint')}</p>
        </div>
        <div class="form-group">
          <label class="form-label">${t('channels.qq_bot_secret')}</label>
          <input class="input" type="password" id="qq-bot-secret" placeholder="..." data-config-section="qq" aria-label="QQ Bot Secret">
          <p class="form-help">${t('channels.qq_bot_secret_hint')}</p>
        </div>
        <div class="form-group">
          <label class="form-label">${t('channels.qq_ws_label')}</label>
          <code style="background:var(--bg-tertiary);padding:6px 12px;border-radius:var(--radius-sm);font-size:0.85rem;">wss://api.sgroup.qq.com/websocket</code>
          <p class="form-help">${t('channels.qq_ws_hint')}</p>
        </div>
        <div style="display:flex;gap:8px;">
          <button class="btn btn-primary" id="save-qq-btn">${t('channels.save_qq')}</button>
          <button class="btn btn-outline test-channel-btn" data-id="qq" style="margin-left:8px;">${t('channels.test_connection')}</button>
        </div>
      </div>
      <div class="card" style="margin-top:16px;">
        <div class="form-group">
          <label class="form-label">🔗 ${t('channels.webhook_label')}</label>
          <p class="form-help" style="margin-bottom:12px;">${t('channels.webhook_hint')}</p>
        </div>
        <div class="form-group">
          <label class="form-label">${t('channels.webhook_path')}</label>
          <input class="input" id="webhook-path" placeholder="my-webhook" data-config-section="webhook" aria-label="${t('channels.webhook_path')}">
          <p class="form-help">${t('channels.webhook_path_hint')}</p>
        </div>
        <div class="form-group">
          <label class="form-label">${t('channels.webhook_full_url')}</label>
          <code id="webhook-full-url" style="background:var(--bg-tertiary);padding:6px 12px;border-radius:var(--radius-sm);font-size:0.85rem;word-break:break-all;">http://你的IP:3000/api/channels/webhook/YOUR_PATH</code>
        </div>
        <div class="form-group">
          <label class="form-label">${t('channels.webhook_secret')}</label>
          <input class="input" type="password" id="webhook-secret" placeholder="可选，用于 HMAC-SHA256 签名验证" data-config-section="webhook" aria-label="${t('channels.webhook_secret')}">
          <p class="form-help">${t('channels.webhook_secret_hint')}</p>
        </div>
        <div class="form-group">
          <label class="form-label">${t('channels.webhook_json_path')}</label>
          <input class="input" id="webhook-json-path" value="message" placeholder="message" data-config-section="webhook" aria-label="${t('channels.webhook_json_path')}">
          <p class="form-help">${t('channels.webhook_json_path_hint')}</p>
        </div>
        <div class="form-group">
          <label class="form-label">${t('channels.webhook_response')}</label>
          <textarea class="input textarea" id="webhook-response-template" rows="2" data-config-section="webhook" aria-label="${t('channels.webhook_response')}">{"reply": "{{reply}}"}</textarea>
          <p class="form-help">${t('channels.webhook_response_hint')}</p>
        </div>
        <div style="display:flex;gap:8px;">
          <button class="btn btn-primary" id="save-webhook-btn">${t('channels.save_webhook')}</button>
          <button class="btn btn-outline" id="copy-webhook-url-btn" style="margin-left:8px;">${t('channels.copy_webhook_url')}</button>
        </div>
      </div>
      <div class="card" style="margin-top:16px;">
        <div class="form-group">
          <label class="form-label">💚 ${t('channels.wechat_label')}</label>
          <p class="form-help" style="margin-bottom:12px;">${t('channels.wechat_hint').replace('企业微信管理后台', '<a href="https://work.weixin.qq.com/" target="_blank" rel="noopener">企业微信管理后台</a>').replace('WeChat Work Admin Console', '<a href="https://work.weixin.qq.com/" target="_blank" rel="noopener">WeChat Work Admin Console</a>')}</p>
        </div>
        <div class="form-group">
          <label class="form-label">${t('channels.wechat_corp_id')}</label>
          <input class="input" id="wx-corp-id" placeholder="ww..." data-config-section="wechat_work" aria-label="${t('channels.wechat_corp_id')}">
          <p class="form-help">${t('channels.wechat_corp_id_hint')}</p>
        </div>
        <div class="form-group">
          <label class="form-label">${t('channels.wechat_corp_secret')}</label>
          <input class="input" type="password" id="wx-corp-secret" placeholder="..." data-config-section="wechat_work" aria-label="${t('channels.wechat_corp_secret')}">
          <p class="form-help">${t('channels.wechat_corp_secret_hint')}</p>
        </div>
        <div class="form-group">
          <label class="form-label">${t('channels.wechat_agent_id')}</label>
          <input class="input" id="wx-agent-id" placeholder="1000001" data-config-section="wechat_work" aria-label="${t('channels.wechat_agent_id')}">
          <p class="form-help">${t('channels.wechat_agent_id_hint')}</p>
        </div>
        <div class="form-group">
          <label class="form-label">${t('channels.wechat_token')}</label>
          <input class="input" id="wx-token" placeholder="自定义Token字符串" data-config-section="wechat_work" aria-label="${t('channels.wechat_token')}">
          <p class="form-help">${t('channels.wechat_token_hint')}</p>
        </div>
        <div class="form-group">
          <label class="form-label">${t('channels.wechat_aes_key')}</label>
          <input class="input" type="password" id="wx-encoding-aes-key" placeholder="43位随机字符串" data-config-section="wechat_work" aria-label="${t('channels.wechat_aes_key')}">
          <p class="form-help">${t('channels.wechat_aes_key_hint')}</p>
        </div>
        <div class="form-group">
          <label class="form-label">${t('channels.wechat_callback_label')}</label>
          <code style="background:var(--bg-tertiary);padding:6px 12px;border-radius:var(--radius-sm);font-size:0.85rem;">http://你的IP:3000/api/channels/wechat_work/callback</code>
          <p class="form-help">${t('channels.wechat_callback_hint')}</p>
        </div>
        <div style="display:flex;gap:8px;">
          <button class="btn btn-primary" id="save-wx-btn">${t('channels.save_wechat')}</button>
          <button class="btn btn-outline test-channel-btn" data-id="wechat_work" style="margin-left:8px;">${t('channels.test_connection')}</button>
        </div>
      </div>
    </div>`;
}

function renderUISection(config) {
  return `
    <div class="config-section">
      <h3 class="config-section-title">🎨 ${t('settings.appearance')}</h3>
      <div class="card">
        <div class="form-group">
          <label class="form-label">${t('settings.theme')}</label>
          <select class="input select" data-config-section="ui" data-key="theme" aria-label="${t('settings.theme')}">
            <option value="dark" ${(config.theme || 'dark') === 'dark' ? 'selected' : ''}>${t('settings.theme_dark')}</option>
            <option value="light" ${config.theme === 'light' ? 'selected' : ''}>${t('settings.theme_light')}</option>
          </select>
        </div>
        <button class="btn btn-primary save-config-btn" data-section="ui">${t('settings.save_appearance')}</button>
      </div>
    </div>`;
}

function renderSystemSection(config) {
  return `
    <div class="config-section">
      <h3 class="config-section-title">⚙️ ${t('settings.system')}</h3>
      <div class="card">
        <div class="form-group">
          <label class="form-label">${t('settings.public_url')}</label>
          <input class="input" data-config-section="system" data-key="public_url" value="${escapeAttr(config.public_url || '')}" placeholder="http://你的公网IP:3000">
          <p class="form-help">${t('settings.public_url_hint')}</p>
        </div>
        <div class="form-group">
          <label class="form-label">${t('settings.language')}</label>
          <select class="input select" id="config-language" data-config-section="system" data-key="language" aria-label="${t('settings.language')}">
            <option value="zh" ${(config.language || 'zh') === 'zh' ? 'selected' : ''}>${t('settings.language_zh')}</option>
            <option value="en" ${config.language === 'en' ? 'selected' : ''}>${t('settings.language_en')}</option>
          </select>
        </div>
        <button class="btn btn-primary save-config-btn" data-section="system">${t('settings.save_system')}</button>
      </div>
    </div>`;
}

function escapeAttr(text) {
  if (!text) return '';
  return text.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

function escapeError(text) {
  if (!text) return '';
  return text.replace(/</g, '&lt;').replace(/>/g, '&gt;');
}
