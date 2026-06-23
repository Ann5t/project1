// ── Onboarding Wizard ──
import { api, showToast } from './api.js';
import { route, navigate } from './app.js';
import { t } from './i18n.js';

route('/onboarding', onboardingPage);

function getSteps() {
  return [
    { id: 'welcome', title: t('onboarding.welcome_title'), description: t('onboarding.welcome_desc'), icon: '👋' },
    { id: 'llm', title: t('onboarding.llm_title'), description: t('onboarding.llm_desc'), icon: '🤖' },
    { id: 'channels', title: t('onboarding.channels_title'), description: t('onboarding.channels_desc'), icon: '📱' },
    { id: 'agent', title: t('onboarding.agent_title'), description: t('onboarding.agent_desc'), icon: '✨' },
    { id: 'done', title: t('onboarding.done_title'), description: t('onboarding.done_desc'), icon: '🚀' },
  ];
}

async function onboardingPage() {
  let currentStep = 0;
  try { const d = await api.config.get('onboarding_step'); currentStep = parseInt(d.value || '0'); } catch {}
  const container = document.createElement('div');
  renderStep(container, currentStep);
  return container;
}

function renderStep(container, stepIndex) {
  const steps = getSteps();
  const step = steps[stepIndex];
  const total = steps.length;

  container.innerHTML = `
    <div class="onboarding-overlay">
      <div class="onboarding-card scale-in">
        <div class="onboarding-step-indicator">
          ${steps.map((s, i) => `<div class="onboarding-dot ${i === stepIndex ? 'active' : ''} ${i < stepIndex ? 'done' : ''}" title="${s.title}" aria-label="${t('onboarding.step')} ${i + 1} ${t('onboarding.of')} ${total}: ${s.title}"></div>`).join('')}
        </div>
        <div class="onboarding-step-counter">${t('onboarding.step')} ${stepIndex + 1} ${t('onboarding.of')} ${total}</div>
        <div style="font-size:2.5rem;margin-bottom:16px;">${step.icon}</div>
        <div class="onboarding-title">${step.title}</div>
        <div class="onboarding-desc">${step.description}</div>
        <div id="onboarding-content">${renderStepContent(stepIndex)}</div>
        <div class="onboarding-actions">
          <div>${stepIndex > 0 ? `<button class="btn btn-secondary btn-sm" id="onb-back">${t('onboarding.prev')}</button>` : ''}</div>
          <div style="display:flex;gap:8px;">
            ${(step.id !== 'welcome' && step.id !== 'done') ? `<button class="btn btn-ghost btn-sm" id="onb-skip">${t('onboarding.skip')}</button>` : ''}
            <button class="btn btn-primary" id="onb-next">${stepIndex === total - 1 ? t('onboarding.start') : t('onboarding.next')}</button>
          </div>
        </div>
      </div>
    </div>`;

  container.querySelector('#onb-next').addEventListener('click', async () => {
    if (stepIndex === total - 1) {
      await api.config.set('onboarding_completed', 'true');
      await api.config.set('onboarding_step', String(stepIndex));
      navigate('/'); return;
    }
    await saveStepData(stepIndex, container);
    await api.config.set('onboarding_step', String(stepIndex + 1));
    renderStep(container, stepIndex + 1);
  });

  container.querySelector('#onb-back')?.addEventListener('click', async () => {
    await api.config.set('onboarding_step', String(stepIndex - 1));
    renderStep(container, stepIndex - 1);
  });

  container.querySelector('#onb-skip')?.addEventListener('click', async () => {
    await api.config.set('onboarding_step', String(stepIndex + 1));
    renderStep(container, stepIndex + 1);
  });
}

function renderStepContent(stepIndex) {
  const steps = getSteps();
  switch (steps[stepIndex].id) {
    case 'welcome':
      return `<div style="background:var(--bg-tertiary);border-radius:var(--radius-md);padding:16px;text-align:left;font-size:0.88rem;line-height:1.8;color:var(--text-secondary);">${t('onboarding.welcome_items')}</div>`;
    case 'llm':
      return `<div class="form-group"><label class="form-label">${t('settings.api_key')}</label><input class="input" type="password" id="onb-api-key" placeholder="sk-..." autocomplete="off"><p class="form-help">${t('settings.api_key_hint').replace('platform.deepseek.com', '<a href="https://platform.deepseek.com/api_keys" target="_blank">platform.deepseek.com</a>')}</p></div><div class="form-group"><label class="form-label">${t('settings.base_url')}</label><input class="input" id="onb-base-url" value="https://api.deepseek.com/v1"></div><div style="display:flex;gap:12px;"><div class="form-group" style="flex:1;"><label class="form-label">${t('settings.flash_model')}</label><select class="input select" id="onb-flash-model"><option value="deepseek-chat">deepseek-chat</option></select></div><div class="form-group" style="flex:1;"><label class="form-label">${t('settings.pro_model')}</label><select class="input select" id="onb-pro-model"><option value="deepseek-reasoner">deepseek-reasoner</option></select></div></div>`;
    case 'channels':
      return `<div class="form-group"><label class="form-label">${t('channels.feishu_label')} ${t('channels.app_id')}</label><input class="input" id="onb-feishu-app-id" placeholder="cli_..."></div><div class="form-group"><label class="form-label">${t('channels.feishu_label')} ${t('channels.app_secret')}</label><input class="input" type="password" id="onb-feishu-secret" placeholder="..."></div><div class="form-group"><label class="form-label">${t('channels.verification_token')}</label><input class="input" id="onb-feishu-token" placeholder="..."></div><p class="form-help">${t('onboarding.channels_hint')}</p>`;
    case 'agent':
      return `<div class="form-group"><label class="form-label">Agent ${t('onboarding.agent_title')}</label><input class="input" id="onb-agent-name" placeholder="e.g. My Assistant..." value="AI Assistant"></div><div class="form-group"><label class="form-label">${t('settings.system_prompt')}</label><textarea class="input textarea" id="onb-system-prompt" rows="3">You are a helpful AI assistant.</textarea></div>`;
    case 'done':
      return `<div style="text-align:center;padding:20px;"><div style="font-size:3rem;margin-bottom:16px;">🚀</div><p style="color:var(--text-secondary);line-height:1.6;">${t('onboarding.done_text')}</p><div style="text-align:left;font-size:0.9rem;line-height:2;color:var(--text-secondary);margin-top:12px;">${t('onboarding.done_items')}</div></div>`;
    default: return '';
  }
}

async function saveStepData(stepIndex, container) {
  const step = getSteps()[stepIndex];
  const updates = {};
  switch (step.id) {
    case 'llm': {
      const apiKey = container.querySelector('#onb-api-key')?.value;
      const baseUrl = container.querySelector('#onb-base-url')?.value;
      const flashModel = container.querySelector('#onb-flash-model')?.value;
      const proModel = container.querySelector('#onb-pro-model')?.value;
      if (apiKey) updates.api_key = apiKey;
      if (baseUrl) updates.base_url = baseUrl;
      if (flashModel) updates.flash_model = flashModel;
      if (proModel) updates.pro_model = proModel;
      break;
    }
    case 'channels': {
      const appId = container.querySelector('#onb-feishu-app-id')?.value;
      const secret = container.querySelector('#onb-feishu-secret')?.value;
      const token = container.querySelector('#onb-feishu-token')?.value;
      if (appId || secret || token) {
        try {
          const channels = await api.channels.list();
          const existing = channels.find(c => c.channel_type === 'feishu');
          const config = { app_id: appId || '', app_secret: secret || '', verification_token: token || '' };
          if (existing) { await api.channels.update(existing.id, { config, enabled: true }); }
          else { await api.channels.create({ channel_type: 'feishu', name: 'Feishu Bot', config }); }
        } catch {}
      }
      break;
    }
    case 'agent': {
      const name = container.querySelector('#onb-agent-name')?.value;
      const sp = container.querySelector('#onb-system-prompt')?.value;
      if (name || sp) updates.system_prompt = sp || '';
      if (name) {
        try { await api.sessions.create({ name, system_prompt: sp || '' }); } catch {}
      }
      break;
    }
  }
  if (Object.keys(updates).length > 0) {
    try { await api.config.updateAll(updates); } catch {}
  }
}
