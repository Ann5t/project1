// ── Tasks Page ──
import { api, showToast, formatDate } from './api.js';
import { route, navigate } from './app.js';

route('/tasks', tasksPage);

// ── Tasks skeleton ──
function tasksSkeleton() {
  return Array(3).fill(0).map(() => `
    <div class="skeleton-card" style="margin-bottom:12px;">
      <div style="display:flex;justify-content:space-between;align-items:flex-start;">
        <div style="flex:1;">
          <div class="skeleton skeleton-text medium"></div>
          <div class="skeleton" style="width:100px;height:18px;border-radius:4px;margin-top:4px;"></div>
        </div>
        <div style="display:flex;gap:8px;">
          <div class="skeleton" style="width:50px;height:22px;border-radius:20px;"></div>
          <div class="skeleton" style="width:28px;height:28px;border-radius:var(--radius-sm);"></div>
          <div class="skeleton" style="width:28px;height:28px;border-radius:var(--radius-sm);"></div>
        </div>
      </div>
      <div class="skeleton skeleton-text long" style="margin-top:8px;"></div>
      <div class="skeleton skeleton-text medium"></div>
    </div>
  `).join('');
}

async function tasksPage() {
  const container = document.createElement('div');
  container.className = 'page';

  container.innerHTML = `
    <div class="page-header" style="display:flex;justify-content:space-between;align-items:center;flex-wrap:wrap;gap:8px;">
      <div>
        <h1 class="page-title">定时任务</h1>
        <p class="page-subtitle">设置定时执行的 AI 任务</p>
      </div>
      <button class="btn btn-primary" id="new-task-btn">+ 新建任务</button>
    </div>
    ${tasksSkeleton()}`;

  try {
    const tasks = await api.tasks.list();
    if (tasks.length === 0) {
      container.innerHTML = `
        <div class="page-header" style="display:flex;justify-content:space-between;align-items:center;flex-wrap:wrap;gap:8px;">
          <div>
            <h1 class="page-title">定时任务</h1>
            <p class="page-subtitle">设置定时执行的 AI 任务</p>
          </div>
          <button class="btn btn-primary" id="new-task-btn">+ 新建任务</button>
        </div>
        <div class="empty-state">
          <div class="empty-state-icon">
            <svg viewBox="0 0 64 64" fill="none" style="width:64px;height:64px;">
              <circle cx="32" cy="32" r="28" stroke="var(--text-tertiary)" stroke-width="2" fill="none"/>
              <path d="M32 14v18M32 36v2" stroke="var(--text-tertiary)" stroke-width="2.5" stroke-linecap="round"/>
              <path d="M32 32l10-6" stroke="var(--accent-primary)" stroke-width="2" stroke-linecap="round"/>
              <path d="M32 32l-8 5" stroke="var(--accent-primary)" stroke-width="2" stroke-linecap="round"/>
            </svg>
          </div>
          <div class="empty-state-title">暂无定时任务</div>
          <div class="empty-state-text">创建定时任务，让 AI 在指定时间自动执行</div>
          <button class="btn btn-primary" id="new-task-empty-btn">创建定时任务</button>
        </div>`;
    } else {
      container.innerHTML += `
        <div style="display:flex;flex-direction:column;gap:12px;">
          ${tasks.map(t => `
            <div class="card">
              <div style="display:flex;justify-content:space-between;align-items:flex-start;">
                <div>
                  <div style="font-weight:600;">${escapeHtml(t.name)}</div>
                  <code style="background:var(--bg-tertiary);padding:2px 8px;border-radius:4px;font-size:0.8rem;margin-top:4px;display:inline-block;">${escapeHtml(t.cron_expression)}</code>
                </div>
                <div style="display:flex;align-items:center;gap:8px;">
                  <span class="badge ${t.enabled ? 'badge-success' : 'badge-error'}">${t.enabled ? '启用' : '禁用'}</span>
                  <button class="btn btn-ghost btn-sm run-task-btn" data-id="${t.id}" title="立即运行" aria-label="立即运行">▶</button>
                  <button class="btn btn-ghost btn-sm delete-task-btn" data-id="${t.id}" title="删除" aria-label="删除" style="color:var(--error);">✕</button>
                </div>
              </div>
              <div style="font-size:0.85rem;color:var(--text-secondary);margin-top:8px;">${escapeHtml((t.prompt || '').substring(0, 100))}${(t.prompt || '').length > 100 ? '...' : ''}</div>
            </div>`).join('')}
        </div>`;
    }
  } catch (e) {
    container.innerHTML += `<div class="empty-state"><div class="empty-state-icon"><svg viewBox="0 0 64 64" fill="none" style="width:64px;height:64px;"><circle cx="32" cy="32" r="30" stroke="var(--error)" stroke-width="2" stroke-dasharray="4 4"/><path d="M32 18v18M32 44v2" stroke="var(--error)" stroke-width="3" stroke-linecap="round"/></svg></div><div class="empty-state-text">加载失败: ${escapeError(e.message)}</div></div>`;
  }

  // Event handlers
  setTimeout(() => {
    const createHandler = () => {
      const name = prompt('任务名称:');
      if (!name) return;
      const cron = prompt('Cron 表达式 (如 "0 9 * * *" = 每天9点):', '0 9 * * *');
      if (!cron) return;
      const promptText = prompt('任务提示词:');
      if (!promptText) return;

      api.tasks.create({ name, cron_expression: cron, prompt: promptText }).then(() => {
        showToast('任务已创建', 'success');
        tasksPage().then(page => container.replaceWith(page));
      }).catch(e => showToast('创建失败: ' + e.message, 'error'));
    };
    container.querySelector('#new-task-btn')?.addEventListener('click', createHandler);
    container.querySelector('#new-task-empty-btn')?.addEventListener('click', createHandler);

    container.querySelectorAll('.run-task-btn').forEach(btn => {
      btn.addEventListener('click', async () => {
        const originalText = btn.textContent;
        btn.disabled = true;
        btn.textContent = '...';
        try {
          await api.tasks.run(btn.dataset.id);
          showToast('任务已触发', 'success');
        } catch (e) {
          showToast('触发失败: ' + e.message, 'error');
        } finally {
          btn.textContent = originalText;
          btn.disabled = false;
        }
      });
    });

    container.querySelectorAll('.delete-task-btn').forEach(btn => {
      btn.addEventListener('click', async () => {
        if (confirm('确认删除此任务？')) {
          try {
            await api.tasks.delete(btn.dataset.id);
            showToast('任务已删除', 'success');
            tasksPage().then(page => container.replaceWith(page));
          } catch (e) {
            showToast('删除失败: ' + e.message, 'error');
          }
        }
      });
    });
  }, 100);

  return container;
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
