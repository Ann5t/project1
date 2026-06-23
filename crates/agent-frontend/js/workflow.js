// ── Workflows Page -- Cytoscape.js DAG Editor ──
import { api, showToast, formatDate } from './api.js';
import { route, navigate } from './app.js';

route('/workflows', workflowsPage);
route('/workflows/:id', workflowDetailPage);

// ── Constants ──
const NODE_TYPES = {
  llm_call:   { label: 'LLM Call',   color: '#8b5cf6', bg: 'rgba(139, 92, 246, 0.18)', icon: '\u{1F916}' },
  tool_call:  { label: 'Tool Call',  color: '#3b82f6', bg: 'rgba(59, 130, 246, 0.18)',  icon: '\u{1F527}' },
  publish:    { label: 'Publish',    color: '#22c55e', bg: 'rgba(34, 197, 94, 0.18)',   icon: '\u{1F4E4}' },
  condition:  { label: 'Condition',  color: '#f59e0b', bg: 'rgba(245, 158, 11, 0.18)',  icon: '\u{1F500}' },
  delay:      { label: 'Delay',      color: '#a78bfa', bg: 'rgba(167, 139, 250, 0.18)',  icon: '\u{23F1}'  },
};

const PRESET_TEMPLATES = {
  'weekly-paper-search': {
    name: 'Weekly Paper Search',
    description: 'Search for recent papers via arxiv, summarize with LLM, publish results.',
    steps: [
      { id: 'tpl-search',    name: 'Search Papers',  type: 'tool_call', config: { tool: 'arxiv_search', query: 'machine learning', max_results: 10 }, position: { x: 400, y: 80 } },
      { id: 'tpl-summarize', name: 'Summarize',      type: 'llm_call',  config: { prompt: 'Summarize the key findings from the search results below. Highlight 3-5 most important papers and their contributions.' }, position: { x: 400, y: 260 } },
      { id: 'tpl-publish',   name: 'Publish Report',  type: 'publish',   config: { channel: 'default', format: 'markdown' }, position: { x: 400, y: 440 } },
    ],
    edges: [
      { id: 'tpl-e1', source: 'tpl-search', target: 'tpl-summarize', label: 'results' },
      { id: 'tpl-e2', source: 'tpl-summarize', target: 'tpl-publish', label: 'report' },
    ],
  },
  'daily-briefing': {
    name: 'Daily Briefing',
    description: 'Generate a daily AI briefing and publish it.',
    steps: [
      { id: 'tpl-llm',    name: 'Generate Briefing', type: 'llm_call', config: { prompt: 'Generate a concise daily briefing covering top tech news, AI developments, and market highlights.' }, position: { x: 400, y: 150 } },
      { id: 'tpl-pub',    name: 'Publish Briefing',  type: 'publish',  config: { channel: 'default', format: 'markdown' }, position: { x: 400, y: 350 } },
    ],
    edges: [
      { id: 'tpl-e1', source: 'tpl-llm', target: 'tpl-pub', label: 'briefing' },
    ],
  },
  'code-review': {
    name: 'Code Review',
    description: 'Get git diff, run LLM code review, publish findings.',
    steps: [
      { id: 'tpl-diff',   name: 'Git Diff',       type: 'tool_call', config: { tool: 'git_diff', repo: '.', staged_only: false }, position: { x: 400, y: 80 } },
      { id: 'tpl-review', name: 'Review Code',     type: 'llm_call',  config: { prompt: 'Review the following code diff for bugs, style issues, and improvements. Provide actionable feedback.' }, position: { x: 400, y: 260 } },
      { id: 'tpl-pub',    name: 'Publish Review',  type: 'publish',   config: { channel: 'default', format: 'markdown' }, position: { x: 400, y: 440 } },
    ],
    edges: [
      { id: 'tpl-e1', source: 'tpl-diff', target: 'tpl-review', label: 'diff' },
      { id: 'tpl-e2', source: 'tpl-review', target: 'tpl-pub', label: 'review' },
    ],
  },
  'stock-briefing': {
    name: 'Stock Market Briefing',
    description: 'Search stock market news, analyze trends with LLM, and publish a market report.',
    steps: [
      { id: 'tpl-search',   name: 'Search Market News', type: 'tool_call', config: { tool: 'web_search', query: 'stock market news today major indices', max_results: 15 }, position: { x: 400, y: 80 } },
      { id: 'tpl-analyze',  name: 'Analyze Trends',     type: 'llm_call',  config: { prompt: 'Analyze the following stock market news. Identify key trends, notable movers, and sector performance. Provide a concise market briefing with actionable insights.' }, position: { x: 400, y: 260 } },
      { id: 'tpl-publish',  name: 'Publish Report',     type: 'publish',   config: { channel: 'default', format: 'markdown' }, position: { x: 400, y: 440 } },
    ],
    edges: [
      { id: 'tpl-e1', source: 'tpl-search', target: 'tpl-analyze', label: 'news' },
      { id: 'tpl-e2', source: 'tpl-analyze', target: 'tpl-publish', label: 'report' },
    ],
  },
  'meeting-notes': {
    name: 'Meeting Notes Summarizer',
    description: 'Read a meeting transcript, extract key points and action items with LLM, then publish the summary.',
    steps: [
      { id: 'tpl-read',     name: 'Read Transcript',    type: 'tool_call', config: { tool: 'read_file', path: '/transcripts/meeting.txt' }, position: { x: 400, y: 80 } },
      { id: 'tpl-summarize',name: 'Summarize Meeting',   type: 'llm_call',  config: { prompt: 'Summarize the following meeting transcript. Extract: 1) Key discussion points, 2) Decisions made, 3) Action items with assignees and deadlines, 4) Follow-up topics. Format as a structured markdown summary.' }, position: { x: 400, y: 260 } },
      { id: 'tpl-publish',  name: 'Publish Notes',      type: 'publish',   config: { channel: 'default', format: 'markdown' }, position: { x: 400, y: 440 } },
    ],
    edges: [
      { id: 'tpl-e1', source: 'tpl-read', target: 'tpl-summarize', label: 'transcript' },
      { id: 'tpl-e2', source: 'tpl-summarize', target: 'tpl-publish', label: 'notes' },
    ],
  },
  'competitor-analysis': {
    name: 'Competitor Analysis',
    description: 'Search for multiple competitors in parallel, then use LLM to compare, contrast, and publish an analysis.',
    steps: [
      { id: 'tpl-search1',  name: 'Search Competitor A', type: 'tool_call', config: { tool: 'web_search', query: 'competitor 1 latest news products', max_results: 10 }, position: { x: 250, y: 80 } },
      { id: 'tpl-search2',  name: 'Search Competitor B', type: 'tool_call', config: { tool: 'web_search', query: 'competitor 2 latest news products', max_results: 10 }, position: { x: 550, y: 80 } },
      { id: 'tpl-compare',  name: 'Compare & Analyze',   type: 'llm_call',  config: { prompt: 'Compare and contrast the following two competitors based on the search results. Analyze: product features, market positioning, recent moves, strengths and weaknesses. Provide a strategic recommendation.' }, position: { x: 400, y: 260 } },
      { id: 'tpl-publish',  name: 'Publish Analysis',    type: 'publish',   config: { channel: 'default', format: 'markdown' }, position: { x: 400, y: 440 } },
    ],
    edges: [
      { id: 'tpl-e1', source: 'tpl-search1', target: 'tpl-compare', label: 'competitor A' },
      { id: 'tpl-e2', source: 'tpl-search2', target: 'tpl-compare', label: 'competitor B' },
      { id: 'tpl-e3', source: 'tpl-compare', target: 'tpl-publish', label: 'analysis' },
    ],
  },
  'learning-plan': {
    name: 'Weekly Learning Plan',
    description: 'Assess current knowledge, search for learning resources, then create a structured weekly learning plan.',
    steps: [
      { id: 'tpl-assess',   name: 'Assess Knowledge',    type: 'llm_call',  config: { prompt: 'Based on current trends and foundational knowledge in AI/ML, identify 3-5 key topics that would be most valuable to learn this week for a software engineer. Consider practical applicability.' }, position: { x: 400, y: 50 } },
      { id: 'tpl-search',   name: 'Find Resources',      type: 'tool_call', config: { tool: 'web_search', query: 'best learning resources tutorials', max_results: 12 }, position: { x: 400, y: 180 } },
      { id: 'tpl-plan',     name: 'Create Plan',         type: 'llm_call',  config: { prompt: 'Create a structured 5-day learning plan for this week based on the topics and resources provided. Include daily goals, estimated time commitment, practice exercises, and milestones. Format as a markdown schedule.' }, position: { x: 400, y: 310 } },
      { id: 'tpl-publish',  name: 'Publish Plan',        type: 'publish',   config: { channel: 'default', format: 'markdown' }, position: { x: 400, y: 440 } },
    ],
    edges: [
      { id: 'tpl-e1', source: 'tpl-assess', target: 'tpl-search', label: 'topics' },
      { id: 'tpl-e2', source: 'tpl-search', target: 'tpl-plan', label: 'resources' },
      { id: 'tpl-e3', source: 'tpl-plan', target: 'tpl-publish', label: 'plan' },
    ],
  },
  'bug-triage': {
    name: 'Bug Report Triage',
    description: 'Read bug reports, categorize by severity with LLM, suggest fixes, and publish a triage report.',
    steps: [
      { id: 'tpl-read',     name: 'Read Bug Reports',    type: 'tool_call', config: { tool: 'read_file', path: '/reports/bugs.json' }, position: { x: 400, y: 50 } },
      { id: 'tpl-categorize',name:'Categorize by Severity',type:'llm_call',  config: { prompt: 'Analyze the following bug reports and categorize each by severity (Critical, High, Medium, Low). Consider impact, frequency, and user exposure. Sort by priority.' }, position: { x: 400, y: 180 } },
      { id: 'tpl-fix',      name: 'Suggest Fixes',       type: 'llm_call',  config: { prompt: 'For each categorized bug, suggest a potential fix or workaround. Estimate the effort required (Small/Medium/Large) and recommend which team or person should handle it.' }, position: { x: 400, y: 310 } },
      { id: 'tpl-publish',  name: 'Publish Triage',      type: 'publish',   config: { channel: 'default', format: 'markdown' }, position: { x: 400, y: 440 } },
    ],
    edges: [
      { id: 'tpl-e1', source: 'tpl-read', target: 'tpl-categorize', label: 'bugs' },
      { id: 'tpl-e2', source: 'tpl-categorize', target: 'tpl-fix', label: 'prioritized' },
      { id: 'tpl-e3', source: 'tpl-fix', target: 'tpl-publish', label: 'triage' },
    ],
  },
};

// ── Helpers ──
function uid() {
  return 'n' + Date.now().toString(36) + '_' + Math.random().toString(36).substring(2, 8);
}

function eid() {
  return 'e' + Date.now().toString(36) + '_' + Math.random().toString(36).substring(2, 8);
}

function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text || '';
  return div.innerHTML;
}

function escapeAttr(text) {
  return (text || '').replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

function escapeError(text) {
  if (!text) return '';
  return text.replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

// ── Skeleton for workflow list ──
function workflowListSkeleton() {
  return Array(3).fill(0).map(() => `
    <div class="skeleton-card" style="margin-bottom:12px;">
      <div style="display:flex;justify-content:space-between;align-items:center;">
        <div style="flex:1;">
          <div class="skeleton skeleton-text medium"></div>
          <div class="skeleton skeleton-text short"></div>
        </div>
        <div style="display:flex;gap:8px;">
          <div class="skeleton" style="width:50px;height:22px;border-radius:20px;"></div>
          <div class="skeleton" style="width:50px;height:22px;border-radius:20px;"></div>
        </div>
      </div>
      <div class="skeleton skeleton-text short" style="margin-top:8px;"></div>
    </div>
  `).join('');
}

// ── Skeleton for workflow editor ──
function workflowEditorSkeleton() {
  return `
    <div style="margin-bottom:16px;">
      <div class="skeleton skeleton-title" style="margin-bottom:8px;"></div>
      <div class="skeleton skeleton-text short"></div>
    </div>
    <div class="skeleton" style="width:100%;height:500px;border-radius:var(--radius-md);"></div>
    <div class="skeleton-card" style="margin-top:24px;">
      <div class="skeleton skeleton-text medium"></div>
      <div class="skeleton skeleton-text short"></div>
    </div>`;
}

// ── List Page ──
async function workflowsPage() {
  const container = document.createElement('div');
  container.className = 'page';
  container.innerHTML = `
    <div class="page-header" style="display:flex;justify-content:space-between;align-items:flex-start;flex-wrap:wrap;gap:12px;">
      <div>
        <h1 class="page-title">🔄 工作流</h1>
        <p class="page-subtitle">创建和编排多步骤 AI 任务</p>
      </div>
      <button class="btn btn-primary" id="new-workflow-btn">+ 新建工作流</button>
    </div>
    ${workflowListSkeleton()}`;

  try {
    const workflows = await api.workflows.list();
    container.innerHTML = `
      <div class="page-header" style="display:flex;justify-content:space-between;align-items:flex-start;flex-wrap:wrap;gap:12px;">
        <div>
          <h1 class="page-title">🔄 工作流</h1>
          <p class="page-subtitle">创建和编排多步骤 AI 任务</p>
        </div>
        <button class="btn btn-primary" id="new-workflow-btn">+ 新建工作流</button>
      </div>`;

    if (workflows.length === 0) {
      container.innerHTML += `
        <div class="empty-state">
          <div class="empty-state-icon">
            <svg viewBox="0 0 64 64" fill="none" style="width:64px;height:64px;">
              <circle cx="18" cy="18" r="6" stroke="var(--accent-primary)" stroke-width="2" fill="none"/>
              <circle cx="32" cy="42" r="6" stroke="#3b82f6" stroke-width="2" fill="none"/>
              <circle cx="46" cy="18" r="6" stroke="#22c55e" stroke-width="2" fill="none"/>
              <path d="M23 22l5 16M36 38l6-16" stroke="var(--text-tertiary)" stroke-width="1.5"/>
            </svg>
          </div>
          <div class="empty-state-title">暂无工作流</div>
          <div class="empty-state-text">创建工作流来自动化复杂任务，如论文搜索、每日简报等</div>
          <button class="btn btn-primary" id="new-workflow-empty-btn">创建第一个工作流</button>
        </div>`;
    } else {
      container.innerHTML += `
        <div style="display:flex;flex-direction:column;gap:12px;">
          ${workflows.map(wf => `
            <div class="card" style="cursor:pointer;" data-wf-id="${escapeAttr(wf.id)}" role="button" tabindex="0">
              <div style="display:flex;justify-content:space-between;align-items:center;">
                <div>
                  <div style="font-weight:600;font-size:1rem;">${escapeHtml(wf.name)}</div>
                  <div style="color:var(--text-secondary);font-size:0.85rem;margin-top:4px;">${escapeHtml(wf.description || '无描述')}</div>
                </div>
                <div style="display:flex;align-items:center;gap:8px;">
                  <span class="badge ${wf.trigger_type === 'cron' ? 'badge-info' : 'badge-warning'}">${wf.trigger_type === 'cron' ? '定时' : '手动'}</span>
                  <span class="badge ${wf.enabled ? 'badge-success' : 'badge-error'}">${wf.enabled ? '启用' : '禁用'}</span>
                </div>
              </div>
              <div style="font-size:0.75rem;color:var(--text-tertiary);margin-top:8px;">
                更新于 ${formatDate(wf.updated_at)}
                ${wf.last_run_at ? ` · 上次运行 ${formatDate(wf.last_run_at)}` : ''}
              </div>
            </div>`).join('')}
        </div>`;

      // Click-to-navigate on cards
      container.querySelectorAll('[data-wf-id]').forEach(card => {
        card.addEventListener('click', () => navigate(`/workflows/${card.dataset.wfId}`));
      });
    }
  } catch (e) {
    container.innerHTML = `
      <div class="page-header"><h1 class="page-title">工作流</h1></div>
      <div class="empty-state"><div class="empty-state-icon"><svg viewBox="0 0 64 64" fill="none" style="width:64px;height:64px;"><circle cx="32" cy="32" r="30" stroke="var(--error)" stroke-width="2" stroke-dasharray="4 4"/><path d="M32 18v18M32 44v2" stroke="var(--error)" stroke-width="3" stroke-linecap="round"/></svg></div><div class="empty-state-text">加载失败: ${escapeError(e.message)}</div></div>`;
  }

  setTimeout(() => {
    // ── Build template selector modal ──
    const allTemplates = Object.entries(PRESET_TEMPLATES);
    const templateCardsHtml = `
      <div class="wf-template-card" data-template="blank" style="border:2px dashed var(--border-color);border-radius:var(--radius-md);padding:14px;cursor:pointer;transition:all 0.2s;text-align:center;">
        <div style="font-size:2rem;margin-bottom:6px;">📄</div>
        <div style="font-weight:600;font-size:0.9rem;margin-bottom:4px;">空白工作流</div>
        <div style="font-size:0.75rem;color:var(--text-tertiary);">从头开始创建自定义工作流</div>
      </div>
      ${allTemplates.map(([key, tpl]) => `
        <div class="wf-template-card" data-template="${escapeAttr(key)}" style="border:1px solid var(--border-color);border-radius:var(--radius-md);padding:14px;cursor:pointer;transition:all 0.2s;">
          <div style="font-weight:600;font-size:0.9rem;margin-bottom:4px;">${escapeHtml(tpl.name)}</div>
          <div style="font-size:0.75rem;color:var(--text-secondary);margin-bottom:6px;line-height:1.4;">${escapeHtml(tpl.description)}</div>
          <div style="font-size:0.7rem;color:var(--text-tertiary);">${tpl.steps.length} 个步骤</div>
        </div>
      `).join('')}
    `;

    const modalHtml = `
      <div class="wf-template-modal-overlay" id="template-modal-overlay" style="display:none;position:fixed;top:0;left:0;right:0;bottom:0;background:rgba(0,0,0,0.55);backdrop-filter:blur(6px);-webkit-backdrop-filter:blur(6px);z-index:10000;align-items:center;justify-content:center;animation:fadeIn 150ms ease-out;">
        <div class="wf-template-modal" style="background:var(--bg-secondary);border:1px solid var(--border-highlight);border-radius:var(--radius-xl);max-width:760px;width:93%;max-height:82vh;overflow-y:auto;box-shadow:0 24px 80px rgba(0,0,0,0.5),0 0 60px rgba(139,92,246,0.08);transform-origin:top center;animation:scaleIn 180ms ease-out;">
          <div style="display:flex;justify-content:space-between;align-items:center;padding:18px 22px;border-bottom:1px solid var(--border-color);position:sticky;top:0;background:var(--bg-secondary);z-index:1;border-radius:var(--radius-xl) var(--radius-xl) 0 0;">
            <h2 style="margin:0;font-size:1.15rem;font-weight:700;">📋 选择工作流模板</h2>
            <button class="btn btn-ghost btn-sm" id="close-template-modal" style="font-size:1.3rem;line-height:1;padding:4px 8px;" title="关闭">✕</button>
          </div>
          <div style="padding:18px 22px 22px;display:grid;grid-template-columns:repeat(auto-fill,minmax(220px,1fr));gap:12px;" id="template-cards-grid">
            ${templateCardsHtml}
          </div>
        </div>
      </div>`;

    container.insertAdjacentHTML('beforeend', modalHtml);

    const overlay = container.querySelector('#template-modal-overlay');
    const showModal = () => { overlay.style.display = 'flex'; };
    const hideModal = () => { overlay.style.display = 'none'; };

    container.querySelector('#close-template-modal')?.addEventListener('click', hideModal);
    overlay?.addEventListener('click', (e) => {
      if (e.target === overlay) hideModal();
    });

    // Handle template card clicks
    container.querySelectorAll('.wf-template-card').forEach(card => {
      card.addEventListener('click', async () => {
        const tplKey = card.dataset.template;
        hideModal();

        const defaultName = tplKey !== 'blank' && PRESET_TEMPLATES[tplKey]
          ? PRESET_TEMPLATES[tplKey].name : '';
        const name = prompt('工作流名称:', defaultName);
        if (!name) return;

        let definition = { steps: [], edges: [] };
        let description = '';

        if (tplKey !== 'blank' && PRESET_TEMPLATES[tplKey]) {
          const tpl = PRESET_TEMPLATES[tplKey];
          description = tpl.description;
          // Generate fresh IDs and map edges
          const idMap = {};
          const steps = tpl.steps.map(s => {
            const newId = uid();
            idMap[s.id] = newId;
            return {
              id: newId,
              name: s.name,
              type: s.type,
              config: s.config || {},
              position: s.position,
            };
          });
          const edges = tpl.edges.map(e => ({
            id: eid(),
            source: idMap[e.source] || '',
            target: idMap[e.target] || '',
            label: e.label || '',
            condition: e.condition || null,
          }));
          definition = { steps, edges };
        }

        try {
          const wf = await api.workflows.create({
            name,
            description,
            definition,
          });
          showToast('工作流已创建', 'success');
          navigate(`/workflows/${wf.id}`);
        } catch (e) {
          showToast('创建失败: ' + e.message, 'error');
        }
      });

      // Hover effect
      card.addEventListener('mouseenter', () => {
        card.style.borderColor = 'var(--accent-primary)';
        card.style.background = 'var(--bg-tertiary)';
        card.style.transform = 'translateY(-2px)';
        card.style.boxShadow = '0 4px 16px rgba(139,92,246,0.12)';
      });
      card.addEventListener('mouseleave', () => {
        card.style.borderColor = card.dataset.template === 'blank' ? 'var(--border-color)' : 'var(--border-color)';
        card.style.background = '';
        card.style.transform = '';
        card.style.boxShadow = '';
      });
    });

    // Escape key to close modal
    container.addEventListener('keydown', (e) => {
      if (e.key === 'Escape' && overlay.style.display === 'flex') {
        hideModal();
      }
    });

    const createHandler = () => showModal();
    container.querySelector('#new-workflow-btn')?.addEventListener('click', createHandler);
    container.querySelector('#new-workflow-empty-btn')?.addEventListener('click', createHandler);
  }, 100);

  return container;
}

// ── Detail / Editor Page ──
async function workflowDetailPage({ id }) {
  const container = document.createElement('div');
  container.className = 'page wf-editor-page';
  container.innerHTML = `<div class="page-header"><h1 class="page-title">工作流编辑器</h1></div>${workflowEditorSkeleton()}`;

  let wf, runs;
  try {
    [wf, runs] = await Promise.all([api.workflows.get(id), api.workflows.runs(id)]);
  } catch (e) {
    container.innerHTML = `
      <div class="page-header"><h1 class="page-title">工作流</h1></div>
      <div class="empty-state"><div class="empty-state-text">加载失败: ${escapeError(e.message)}</div></div>`;
    return container;
  }

  // Parse definition; ensure shape
  const definition = wf.definition && typeof wf.definition === 'object' ? wf.definition : { steps: [], edges: [] };
  if (!Array.isArray(definition.steps)) definition.steps = [];
  if (!Array.isArray(definition.edges)) definition.edges = [];

  // ── Build Layout ──
  container.innerHTML = `
    <div class="page-header" style="display:flex;justify-content:space-between;align-items:flex-start;flex-wrap:wrap;gap:12px;margin-bottom:16px;">
      <div style="flex:1;min-width:200px;">
        <h1 class="page-title" id="wf-title">${escapeHtml(wf.name)}</h1>
        <p class="page-subtitle" id="wf-desc">${escapeHtml(wf.description || '')} · ${wf.trigger_type === 'cron' ? '定时触发' : '手动触发'}</p>
      </div>
      <div style="display:flex;gap:8px;flex-wrap:wrap;align-items:center;" id="wf-toolbar-top">
        <button class="btn btn-primary btn-sm" id="run-wf-btn" title="Ctrl+Enter">▶ 运行</button>
        <button class="btn btn-secondary btn-sm" id="save-wf-btn" title="Ctrl+S">💾 保存</button>
        <div style="position:relative;display:inline-block;">
          <button class="btn btn-secondary btn-sm" id="template-btn">📋 模板</button>
          <div class="wf-dropdown-menu" id="template-menu" style="display:none;">
            ${Object.entries(PRESET_TEMPLATES).map(([key, tpl]) =>
              `<button class="wf-dropdown-item" data-template="${key}">${escapeHtml(tpl.name)}<br><small style="color:var(--text-tertiary);font-size:0.75rem;">${escapeHtml(tpl.description)}</small></button>`
            ).join('')}
          </div>
        </div>
        <button class="btn btn-ghost btn-sm" id="delete-wf-btn" style="color:var(--error);">删除</button>
      </div>
    </div>

    <!-- Editor area: toolbar + canvas + panel -->
    <div class="wf-editor-layout">
      <div class="wf-canvas-wrapper">
        <div class="wf-canvas-toolbar" id="wf-ntoolbar">
          <div style="display:flex;gap:4px;align-items:center;flex-wrap:wrap;">
            <div style="position:relative;display:inline-block;">
              <button class="btn btn-primary btn-sm" id="add-node-btn">+ 添加节点</button>
              <div class="wf-dropdown-menu" id="add-node-menu" style="display:none;">
                ${Object.entries(NODE_TYPES).map(([type, info]) =>
                  `<button class="wf-dropdown-item" data-node-type="${type}">
                    <span style="display:inline-block;width:24px;">${info.icon}</span> ${info.label}
                  </button>`
                ).join('')}
              </div>
            </div>
            <button class="btn btn-secondary btn-sm" id="connect-mode-btn" title="C">🔗 连线模式</button>
            <button class="btn btn-ghost btn-sm" id="fit-btn" title="F 适应画布">🔍</button>
            <button class="btn btn-ghost btn-sm" id="reflow-btn" title="L 自动布局">🔃</button>
            <span style="margin-left:12px;font-size:0.8rem;color:var(--text-tertiary);" id="wf-status-label">
              节点: ${definition.steps.length} | 连线: ${definition.edges.length}
            </span>
          </div>
          <div style="display:flex;gap:4px;align-items:center;">
            <span id="connect-mode-indicator" style="display:none;color:var(--accent-primary);font-weight:600;font-size:0.8rem;animation:pulse 1.2s ease-in-out infinite;">
              ● 连线模式: 点击源节点，再点击目标节点
            </span>
          </div>
        </div>
        <div id="cy-container" style="width:100%;height:500px;border:1px solid var(--border-color);border-radius:var(--radius-md);background:var(--bg-tertiary);position:relative;"></div>
      </div>

      <!-- Property Panel -->
      <div class="wf-property-panel" id="wf-property-panel">
        <div style="padding:16px;border-bottom:1px solid var(--border-color);display:flex;justify-content:space-between;align-items:center;">
          <h3 style="font-size:0.9rem;font-weight:600;">属性面板</h3>
          <button class="btn btn-ghost btn-sm" id="close-panel-btn" style="display:none;">✕</button>
        </div>
        <div id="wf-panel-content" style="padding:16px;overflow-y:auto;flex:1;">
          <div style="text-align:center;color:var(--text-tertiary);padding:20px 0;">
            <div style="font-size:2rem;margin-bottom:8px;">💡</div>
            <div style="font-size:0.85rem;">点击节点查看和编辑属性<br>点击连线修改标签</div>
          </div>
        </div>
      </div>
    </div>

    <!-- Execution history -->
    <div class="config-section" style="margin-top:24px;">
      <h3 class="config-section-title">📋 执行历史</h3>
      <div id="wf-history-container">
        ${renderHistory(runs)}
      </div>
    </div>`;

  // ── Cytoscape Init ──
  let cy;
  let connectMode = false;
  let connectSource = null;
  let selectedElement = null;

  function initCy() {
    const cyEl = container.querySelector('#cy-container');
    if (!cyEl) return;

    // Prepare elements
    const elements = [];
    const stepMap = new Map();

    for (const step of definition.steps) {
      const info = NODE_TYPES[step.type] || NODE_TYPES.llm_call;
      const x = step.position?.x ?? 200 + Math.random() * 400;
      const y = step.position?.y ?? 100 + Math.random() * 300;
      elements.push({
        group: 'nodes',
        data: {
          id: step.id,
          label: step.name || info.label,
          type: step.type || 'llm_call',
          config: step.config || {},
          icon: info.icon,
        },
        position: { x, y },
      });
      stepMap.set(step.id, step);
    }

    for (const edge of definition.edges) {
      elements.push({
        group: 'edges',
        data: {
          id: edge.id || eid(),
          source: edge.source,
          target: edge.target,
          label: edge.label || '',
          condition: edge.condition || null,
        },
      });
    }

    cy = cytoscape({
      container: cyEl,
      elements: elements,
      style: buildCyStyle(),
      layout: elements.length > 1
        ? { name: 'breadthfirst', directed: true, spacingFactor: 1.2, animate: true, animationDuration: 400 }
        : { name: 'preset' },
      wheelSensitivity: 0.3,
      minZoom: 0.2,
      maxZoom: 3,
    });

    // Minimap
    if (typeof cy.minimap === 'function') {
      try {
        const mm = cy.minimap({ container: cyEl, height: 140, width: 180 });
      } catch (_) { /* minimap unavailable */ }
    }

    // ── Events ──
    cy.on('tap', 'node', (evt) => {
      if (connectMode) {
        if (!connectSource) {
          connectSource = evt.target;
          evt.target.addClass('connect-source');
          updateConnectIndicator();
        } else {
          const target = evt.target;
          if (connectSource.id() !== target.id()) {
            const existing = cy.edges().filter(e =>
              e.data('source') === connectSource.id() && e.data('target') === target.id()
            );
            if (existing.length === 0) {
              cy.add({
                group: 'edges',
                data: {
                  id: eid(),
                  source: connectSource.id(),
                  target: target.id(),
                  label: '',
                  condition: null,
                },
              });
              updateStatusLabel();
            } else {
              showToast('连线已存在', 'warning');
            }
          }
          connectSource.removeClass('connect-source');
          connectSource = null;
          setConnectMode(false);
          updateConnectIndicator();
        }
        return;
      }
      selectNode(evt.target);
    });

    cy.on('tap', 'edge', (evt) => {
      if (connectMode) return;
      selectEdge(evt.target);
    });

    cy.on('tap', (evt) => {
      if (evt.target === cy) {
        if (connectMode) {
          if (connectSource) {
            connectSource.removeClass('connect-source');
            connectSource = null;
            updateConnectIndicator();
          }
        }
        deselectAll();
      }
    });

    updateStatusLabel();
  }

  function buildCyStyle() {
    const style = [
      {
        selector: 'node',
        style: {
          'background-color': '#1a1a24',
          'label': 'data(label)',
          'color': '#f0f0f5',
          'font-size': '11px',
          'font-family': "'DM Sans', 'Noto Sans SC', sans-serif",
          'font-weight': '500',
          'text-valign': 'bottom',
          'text-halign': 'center',
          'text-margin-y': 8,
          'text-wrap': 'wrap',
          'text-max-width': '100px',
          'width': 50,
          'height': 50,
          'border-width': 2,
          'border-color': '#3b3b52',
          'shape': 'round-rectangle',
          'transition-property': 'border-color, border-width, background-color, width, height',
          'transition-duration': '200ms',
          'text-background-color': '#111118',
          'text-background-opacity': 0.7,
          'text-background-padding': '3px',
          'text-background-shape': 'round-rectangle',
        },
      },
      ...Object.entries(NODE_TYPES).map(([type, info]) => ({
        selector: `node[type="${type}"]`,
        style: {
          'background-color': info.bg,
          'border-color': info.color,
          'label': `data(label)`,
        },
      })),
      {
        selector: 'node:selected',
        style: {
          'border-width': 3,
          'border-color': '#8b5cf6',
          'width': 54,
          'height': 54,
          'shadow-blur': 16,
          'shadow-color': 'rgba(139, 92, 246, 0.6)',
          'shadow-opacity': 1,
          'shadow-offset-x': 0,
          'shadow-offset-y': 0,
        },
      },
      {
        selector: 'node.connect-source',
        style: {
          'border-width': 4,
          'border-color': '#f59e0b',
          'shadow-blur': 20,
          'shadow-color': 'rgba(245, 158, 11, 0.7)',
          'shadow-opacity': 1,
        },
      },
      {
        selector: 'node:active',
        style: {
          'overlay-color': '#8b5cf6',
          'overlay-opacity': 0.12,
          'overlay-padding': 8,
        },
      },
      {
        selector: 'edge',
        style: {
          'width': 2,
          'line-color': '#4a4a68',
          'target-arrow-color': '#4a4a68',
          'target-arrow-shape': 'triangle',
          'curve-style': 'bezier',
          'arrow-scale': 1.2,
          'label': 'data(label)',
          'color': '#a0a0b8',
          'font-size': '10px',
          'font-family': "'DM Sans', 'Noto Sans SC', sans-serif",
          'text-background-color': '#1a1a24',
          'text-background-opacity': 0.9,
          'text-background-padding': '3px',
          'text-background-shape': 'round-rectangle',
          'transition-property': 'line-color, target-arrow-color, width',
          'transition-duration': '200ms',
        },
      },
      {
        selector: 'edge:selected',
        style: {
          'width': 3,
          'line-color': '#8b5cf6',
          'target-arrow-color': '#8b5cf6',
        },
      },
      {
        selector: 'edge.connect-preview',
        style: {
          'line-color': '#f59e0b',
          'target-arrow-color': '#f59e0b',
          'line-style': 'dashed',
          'width': 2.5,
        },
      },
    ];
    return style;
  }

  // ── Selection handling ──
  function selectNode(node) {
    cy.elements().unselect();
    node.select();
    selectedElement = node;
    renderNodePanel(node);
  }

  function selectEdge(edge) {
    cy.elements().unselect();
    edge.select();
    selectedElement = edge;
    renderEdgePanel(edge);
  }

  function deselectAll() {
    cy.elements().unselect();
    selectedElement = null;
    renderEmptyPanel();
  }

  // ── Panel rendering ──
  const panelContent = container.querySelector('#wf-panel-content');
  const closePanelBtn = container.querySelector('#close-panel-btn');

  closePanelBtn?.addEventListener('click', deselectAll);

  function renderNodePanel(node) {
    const data = node.data();
    const info = NODE_TYPES[data.type] || NODE_TYPES.llm_call;
    closePanelBtn.style.display = '';

    panelContent.innerHTML = `
      <div class="form-group">
        <label class="form-label">节点类型</label>
        <span class="badge" style="background:${info.bg};color:${info.color};font-size:0.8rem;">${info.icon} ${info.label}</span>
      </div>
      <div class="form-group">
        <label class="form-label">名称</label>
        <input class="input" id="pn-name" value="${escapeAttr(data.label)}" placeholder="节点名称" aria-label="节点名称">
      </div>
      <div class="form-group">
        <label class="form-label">类型</label>
        <select class="input select" id="pn-type" aria-label="节点类型">
          ${Object.entries(NODE_TYPES).map(([t, inf]) =>
            `<option value="${t}" ${data.type === t ? 'selected' : ''}>${inf.icon} ${inf.label}</option>`
          ).join('')}
        </select>
      </div>
      <div class="form-group">
        <label class="form-label">配置 (JSON)</label>
        <textarea class="input textarea" id="pn-config" rows="6" style="font-family:var(--font-mono);font-size:0.78rem;" aria-label="节点配置">${escapeHtml(JSON.stringify(data.config, null, 2))}</textarea>
      </div>
      <button class="btn btn-danger btn-sm" id="pn-delete-node" style="width:100%;" title="Delete">删除节点</button>
      <div style="margin-top:8px;font-size:0.75rem;color:var(--text-tertiary);">
        ID: ${escapeHtml(data.id)}
      </div>`;

    panelContent.querySelector('#pn-name')?.addEventListener('input', (e) => {
      node.data('label', e.target.value);
      updateStatusLabel();
    });
    panelContent.querySelector('#pn-type')?.addEventListener('change', (e) => {
      node.data('type', e.target.value);
      node.data('icon', NODE_TYPES[e.target.value]?.icon || '');
      updateStatusLabel();
    });
    panelContent.querySelector('#pn-config')?.addEventListener('input', () => {
      try {
        const cfg = JSON.parse(panelContent.querySelector('#pn-config').value);
        node.data('config', cfg);
      } catch (_) { /* invalid JSON */ }
      updateStatusLabel();
    });
    panelContent.querySelector('#pn-delete-node')?.addEventListener('click', () => {
      if (confirm('确认删除此节点及其所有连线？')) {
        cy.remove(node);
        deselectAll();
        updateStatusLabel();
      }
    });
  }

  function renderEdgePanel(edge) {
    const data = edge.data();
    closePanelBtn.style.display = '';

    panelContent.innerHTML = `
      <div class="form-group">
        <label class="form-label">源节点</label>
        <div style="font-size:0.85rem;color:var(--text-secondary);">${escapeHtml(data.source)}</div>
      </div>
      <div class="form-group">
        <label class="form-label">目标节点</label>
        <div style="font-size:0.85rem;color:var(--text-secondary);">${escapeHtml(data.target)}</div>
      </div>
      <div class="form-group">
        <label class="form-label">标签</label>
        <input class="input" id="pn-edge-label" value="${escapeAttr(data.label || '')}" placeholder="e.g. 'results', 'on success'" aria-label="连线标签">
      </div>
      <div class="form-group">
        <label class="form-label">条件 (可选)</label>
        <input class="input" id="pn-edge-condition" value="${escapeAttr(data.condition || '')}" placeholder="e.g. status == 'success'" aria-label="连线条件">
      </div>
      <button class="btn btn-danger btn-sm" id="pn-delete-edge" style="width:100%;" title="Delete">删除连线</button>
      <div style="margin-top:8px;font-size:0.75rem;color:var(--text-tertiary);">
        ID: ${escapeHtml(data.id)}
      </div>`;

    panelContent.querySelector('#pn-edge-label')?.addEventListener('input', (e) => {
      edge.data('label', e.target.value);
      updateStatusLabel();
    });
    panelContent.querySelector('#pn-edge-condition')?.addEventListener('input', (e) => {
      edge.data('condition', e.target.value || null);
      updateStatusLabel();
    });
    panelContent.querySelector('#pn-delete-edge')?.addEventListener('click', () => {
      cy.remove(edge);
      deselectAll();
      updateStatusLabel();
    });
  }

  function renderEmptyPanel() {
    closePanelBtn.style.display = 'none';
    panelContent.innerHTML = `
      <div style="text-align:center;color:var(--text-tertiary);padding:20px 0;">
        <div style="font-size:2rem;margin-bottom:8px;">💡</div>
        <div style="font-size:0.85rem;">点击节点查看和编辑属性<br>点击连线修改标签</div>
      </div>`;
  }

  // ── Connect mode ──
  function setConnectMode(on) {
    connectMode = on;
    if (!on && connectSource) {
      connectSource.removeClass('connect-source');
      connectSource = null;
    }
    const btn = container.querySelector('#connect-mode-btn');
    if (btn) {
      btn.textContent = on ? '🔗 连线中...' : '🔗 连线模式';
      btn.className = on ? 'btn btn-primary btn-sm' : 'btn btn-secondary btn-sm';
    }
    updateConnectIndicator();
  }

  function updateConnectIndicator() {
    const indicator = container.querySelector('#connect-mode-indicator');
    if (indicator) {
      indicator.style.display = connectMode ? '' : 'none';
      if (connectMode && connectSource) {
        indicator.textContent = `🟡 源: "${connectSource.data('label')}" — 请点击目标节点`;
      } else if (connectMode) {
        indicator.textContent = '● 连线模式: 点击源节点，再点击目标节点';
      }
    }
  }

  // ── Status ──
  function updateStatusLabel() {
    const lbl = container.querySelector('#wf-status-label');
    if (lbl) {
      lbl.textContent = `节点: ${cy.nodes().length} | 连线: ${cy.edges().length}`;
    }
  }

  // ── Build graph JSON ──
  function buildDefinition() {
    const steps = cy.nodes().map(n => ({
      id: n.data('id'),
      name: n.data('label') || '',
      type: n.data('type') || 'llm_call',
      config: n.data('config') || {},
      position: { x: Math.round(n.position('x')), y: Math.round(n.position('y')) },
    }));
    const edges = cy.edges().map(e => ({
      id: e.data('id'),
      source: e.data('source'),
      target: e.data('target'),
      label: e.data('label') || undefined,
      condition: e.data('condition') || undefined,
    }));
    return { steps, edges };
  }

  // ── Initialize Cy after container is in DOM ──
  requestAnimationFrame(() => { initCy(); });

  // ── Button handlers ──
  const saveWorkflow = async () => {
    if (!cy) return;
    try {
      const definition = buildDefinition();
      const btn = container.querySelector('#save-wf-btn');
      const orig = btn.textContent;
      btn.disabled = true;
      btn.textContent = '保存中...';
      await api.workflows.update(id, { definition });
      btn.textContent = orig;
      btn.disabled = false;
      showToast('工作流已保存', 'success');
    } catch (e) {
      const btn = container.querySelector('#save-wf-btn');
      if (btn) { btn.textContent = '💾 保存'; btn.disabled = false; }
      showToast('保存失败: ' + e.message, 'error');
    }
  };

  container.querySelector('#save-wf-btn')?.addEventListener('click', saveWorkflow);

  container.querySelector('#run-wf-btn')?.addEventListener('click', async () => {
    try {
      if (cy) {
        const definition = buildDefinition();
        await api.workflows.update(id, { definition });
      }
      const btn = container.querySelector('#run-wf-btn');
      const orig = btn.textContent;
      btn.disabled = true;
      btn.textContent = '运行中...';
      showToast('工作流已开始运行...', 'info');
      const result = await api.workflows.run(id);
      btn.textContent = orig;
      btn.disabled = false;
      showToast(`运行完成: ${result.status}`, result.status === 'success' ? 'success' : 'error');
      // Refresh history
      const freshRuns = await api.workflows.runs(id);
      const histEl = container.querySelector('#wf-history-container');
      if (histEl) histEl.innerHTML = renderHistory(freshRuns);
    } catch (e) {
      showToast('运行失败: ' + e.message, 'error');
      const btn = container.querySelector('#run-wf-btn');
      if (btn) { btn.textContent = '▶ 运行'; btn.disabled = false; }
    }
  });

  container.querySelector('#delete-wf-btn')?.addEventListener('click', async () => {
    if (confirm('确认删除此工作流？此操作不可撤销。')) {
      try {
        await api.workflows.delete(id);
        showToast('工作流已删除', 'success');
        navigate('/workflows');
      } catch (e) {
        showToast('删除失败: ' + e.message, 'error');
      }
    }
  });

  // Add node dropdown
  container.querySelector('#add-node-btn')?.addEventListener('click', (e) => {
    e.stopPropagation();
    const menu = container.querySelector('#add-node-menu');
    menu.style.display = menu.style.display === 'none' ? '' : 'none';
  });
  container.querySelectorAll('#add-node-menu .wf-dropdown-item').forEach(item => {
    item.addEventListener('click', () => {
      if (!cy) return;
      const nodeType = item.dataset.nodeType;
      const info = NODE_TYPES[nodeType] || NODE_TYPES.llm_call;
      const extent = cy.extent();
      const cx = (extent.x1 + extent.x2) / 2;
      const cy_y = (extent.y1 + extent.y2) / 2;
      const newNode = cy.add({
        group: 'nodes',
        data: {
          id: uid(),
          label: info.label,
          type: nodeType,
          config: {},
          icon: info.icon,
        },
        position: { x: cx + (Math.random() - 0.5) * 40, y: cy_y + (Math.random() - 0.5) * 40 },
      });
      container.querySelector('#add-node-menu').style.display = 'none';
      updateStatusLabel();
      selectNode(newNode);
    });
  });

  // Connect mode toggle
  container.querySelector('#connect-mode-btn')?.addEventListener('click', () => {
    setConnectMode(!connectMode);
  });

  // Fit button
  container.querySelector('#fit-btn')?.addEventListener('click', () => {
    if (cy) cy.fit(undefined, 40);
  });

  // Reflow button
  container.querySelector('#reflow-btn')?.addEventListener('click', () => {
    if (cy && cy.nodes().length > 1) {
      cy.layout({
        name: 'breadthfirst',
        directed: true,
        spacingFactor: 1.2,
        animate: true,
        animationDuration: 500,
      }).run();
    }
  });

  // Template dropdown
  container.querySelector('#template-btn')?.addEventListener('click', (e) => {
    e.stopPropagation();
    const menu = container.querySelector('#template-menu');
    menu.style.display = menu.style.display === 'none' ? '' : 'none';
  });
  container.querySelectorAll('#template-menu .wf-dropdown-item').forEach(item => {
    item.addEventListener('click', () => {
      if (!cy) return;
      const tplKey = item.dataset.template;
      const tpl = PRESET_TEMPLATES[tplKey];
      if (!tpl) return;
      if (!confirm(`加载模板 "${tpl.name}"？当前工作流将被替换。`)) return;

      cy.elements().remove();

      for (const step of tpl.steps) {
        cy.add({
          group: 'nodes',
          data: {
            id: uid(),
            label: step.name,
            type: step.type,
            config: step.config || {},
            icon: NODE_TYPES[step.type]?.icon || '',
          },
          position: step.position || { x: 400, y: 80 + tpl.steps.indexOf(step) * 180 },
        });
      }

      const nodeIds = cy.nodes().map(n => n.id());
      for (const edge of tpl.edges) {
        const tplStepIds = tpl.steps.map(s => s.id);
        const sourceIdx = tplStepIds.indexOf(edge.source);
        const targetIdx = tplStepIds.indexOf(edge.target);
        if (sourceIdx >= 0 && targetIdx >= 0 && sourceIdx < nodeIds.length && targetIdx < nodeIds.length) {
          cy.add({
            group: 'edges',
            data: {
              id: eid(),
              source: nodeIds[sourceIdx],
              target: nodeIds[targetIdx],
              label: edge.label || '',
              condition: edge.condition || null,
            },
          });
        }
      }

      updateStatusLabel();
      deselectAll();
      cy.layout({
        name: 'breadthfirst',
        directed: true,
        spacingFactor: 1.2,
        animate: true,
        animationDuration: 500,
      }).run();
      container.querySelector('#template-menu').style.display = 'none';
      showToast(`模板 "${tpl.name}" 已加载`, 'success');
    });
  });

  // Close dropdowns on outside click
  container.addEventListener('click', (e) => {
    if (!e.target.closest('#add-node-btn') && !e.target.closest('#add-node-menu')) {
      const menu = container.querySelector('#add-node-menu');
      if (menu) menu.style.display = 'none';
    }
    if (!e.target.closest('#template-btn') && !e.target.closest('#template-menu')) {
      const menu = container.querySelector('#template-menu');
      if (menu) menu.style.display = 'none';
    }
  });

  // ── Keyboard shortcuts for workflow editor ──
  container.addEventListener('keydown', (e) => {
    const tag = document.activeElement?.tagName;
    const isInput = tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT';
    if (isInput) return;

    // Ctrl+S = Save
    if ((e.ctrlKey || e.metaKey) && (e.key === 's' || e.key === 'S')) {
      e.preventDefault();
      saveWorkflow();
    }
    // Ctrl+Enter = Run
    if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
      e.preventDefault();
      container.querySelector('#run-wf-btn')?.click();
    }
    // Delete = Delete selected
    if (e.key === 'Delete' && selectedElement && !e.ctrlKey && !e.metaKey) {
      e.preventDefault();
      if (selectedElement.isNode()) {
        if (confirm('确认删除此节点及其所有连线？')) {
          cy.remove(selectedElement);
          deselectAll();
          updateStatusLabel();
        }
      } else if (selectedElement.isEdge()) {
        cy.remove(selectedElement);
        deselectAll();
        updateStatusLabel();
      }
    }
    // F = Fit
    if (e.key === 'f' && !e.ctrlKey && !e.metaKey) {
      e.preventDefault();
      if (cy) cy.fit(undefined, 40);
    }
    // L = Layout
    if (e.key === 'l' && !e.ctrlKey && !e.metaKey) {
      e.preventDefault();
      if (cy && cy.nodes().length > 1) {
        cy.layout({
          name: 'breadthfirst',
          directed: true,
          spacingFactor: 1.2,
          animate: true,
          animationDuration: 500,
        }).run();
      }
    }
    // C = Connect mode toggle
    if (e.key === 'c' && !e.ctrlKey && !e.metaKey) {
      e.preventDefault();
      setConnectMode(!connectMode);
    }
    // Escape = Deselect / Cancel connect mode
    if (e.key === 'Escape') {
      if (connectMode) {
        setConnectMode(false);
      } else {
        deselectAll();
      }
    }
  });

  return container;
}

// ── History renderer ──
function renderHistory(runs) {
  if (!runs || runs.length === 0) {
    return '<div class="empty-state" style="padding:24px;"><div class="empty-state-text">暂无执行记录 -- 点击"运行"开始</div></div>';
  }
  return runs.map(r => {
    const statusBadge = r.status === 'success' ? 'badge-success'
      : r.status === 'error' ? 'badge-error'
      : r.status === 'running' ? 'badge-info'
      : 'badge-warning';
    const statusIcon = r.status === 'success' ? '✅'
      : r.status === 'error' ? '❌'
      : r.status === 'running' ? '⏳'
      : '⚠';
    return `
      <div class="card" style="margin-bottom:8px;padding:12px 20px;">
        <div style="display:flex;justify-content:space-between;align-items:center;">
          <div style="display:flex;align-items:center;gap:10px;">
            <span class="badge ${statusBadge}">${statusIcon} ${escapeHtml(r.status)}</span>
            ${r.duration_ms ? `<span style="font-size:0.8rem;color:var(--text-tertiary);">${(r.duration_ms / 1000).toFixed(1)}s</span>` : ''}
          </div>
          <span style="font-size:0.8rem;color:var(--text-tertiary);">${formatDate(r.started_at)}</span>
        </div>
        ${r.error ? `<div style="margin-top:6px;font-size:0.82rem;color:var(--error);">${escapeHtml(r.error)}</div>` : ''}
        ${r.publish_url ? `<div style="margin-top:6px;"><a href="${escapeAttr(r.publish_url)}" target="_blank" rel="noopener">📄 查看发布结果</a></div>` : ''}
        ${r.result ? `<details style="margin-top:6px;"><summary style="cursor:pointer;font-size:0.82rem;color:var(--text-secondary);">查看结果</summary><pre style="margin-top:6px;background:var(--bg-tertiary);padding:10px;border-radius:var(--radius-sm);font-size:0.75rem;overflow-x:auto;max-height:200px;">${escapeHtml(typeof r.result === 'string' ? r.result : JSON.stringify(r.result, null, 2))}</pre></details>` : ''}
      </div>`;
  }).join('');
}
