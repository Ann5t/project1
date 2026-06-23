// ── Chat Page ──
import { api, showToast, formatDate } from './api.js';
import { route } from './app.js';
import { t } from './i18n.js';

route('/', chatPage);
route('/chat/:id', chatPageWithSession);

async function chatPage() {
  return chatPageWithSession({ id: null });
}

// ── Skeleton loader for chat messages ──
function renderChatSkeleton() {
  return `
    <div style="padding:24px;display:flex;flex-direction:column;gap:16px;">
      <div style="display:flex;gap:12px;align-items:flex-start;">
        <div class="skeleton skeleton-avatar"></div>
        <div class="skeleton-card" style="flex:1;max-width:320px;">
          <div class="skeleton skeleton-text medium"></div>
          <div class="skeleton skeleton-text short"></div>
        </div>
      </div>
      <div style="display:flex;gap:12px;align-items:flex-start;flex-direction:row-reverse;">
        <div class="skeleton skeleton-avatar"></div>
        <div class="skeleton-card" style="flex:1;max-width:240px;">
          <div class="skeleton skeleton-text long"></div>
        </div>
      </div>
      <div style="display:flex;gap:12px;align-items:flex-start;">
        <div class="skeleton skeleton-avatar"></div>
        <div class="skeleton-card" style="flex:1;max-width:400px;">
          <div class="skeleton skeleton-text long"></div>
          <div class="skeleton skeleton-text medium"></div>
          <div class="skeleton skeleton-text short"></div>
        </div>
      </div>
    </div>`;
}

// ── Render messages into a container ──
function renderMessages(container, messages) {
  if (messages.length === 0) {
    container.innerHTML = `
      <div class="empty-state">
        <div class="empty-state-icon">
          <svg viewBox="0 0 64 64" fill="none" style="width:64px;height:64px;">
            <rect x="8" y="10" width="48" height="40" rx="8" stroke="var(--text-tertiary)" stroke-width="2" fill="none"/>
            <path d="M14 22h18M14 30h28M14 38h16" stroke="var(--text-tertiary)" stroke-width="2" stroke-linecap="round"/>
            <circle cx="48" cy="42" r="10" fill="var(--accent-primary)" opacity="0.8"/>
            <path d="M45 42l2 2 4-4" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </div>
        <div class="empty-state-title">${t('chat.start_conversation')}</div>
        <div class="empty-state-text">${t('chat.start_hint')}</div>
      </div>`;
  } else {
    container.innerHTML = '';
    messages.forEach(msg => {
      container.appendChild(createMessageBubble(msg));
    });
  }
}

async function chatPageWithSession({ id }) {
  const container = document.createElement('div');
  container.className = 'chat-container';

  // Messages area (initially with skeleton if loading a session)
  const messagesDiv = document.createElement('div');
  messagesDiv.className = 'chat-messages';
  messagesDiv.setAttribute('role', 'log');
  messagesDiv.setAttribute('aria-live', 'polite');
  messagesDiv.setAttribute('aria-label', '对话消息区域');

  if (id) {
    messagesDiv.innerHTML = renderChatSkeleton();
  } else {
    messagesDiv.innerHTML = `
      <div class="empty-state">
        <div class="empty-state-icon">
          <svg viewBox="0 0 64 64" fill="none" style="width:64px;height:64px;">
            <rect x="8" y="10" width="48" height="40" rx="8" stroke="var(--text-tertiary)" stroke-width="2" fill="none"/>
            <path d="M14 22h18M14 30h28M14 38h16" stroke="var(--text-tertiary)" stroke-width="2" stroke-linecap="round"/>
            <circle cx="48" cy="42" r="10" fill="var(--accent-primary)" opacity="0.8"/>
            <path d="M45 42l2 2 4-4" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </div>
        <div class="empty-state-title">${t('chat.start_conversation')}</div>
        <div class="empty-state-text">${t('chat.start_hint')}</div>
      </div>`;
  }

  // Input area (always visible immediately)
  const inputArea = document.createElement('div');
  inputArea.className = 'chat-input-area';
  inputArea.innerHTML = `
    <div class="chat-input-row">
      <textarea class="chat-input" id="chat-input" placeholder="${t('chat.type_message')}" rows="1" aria-label="${t('chat.send')}"></textarea>
      <button class="chat-send-btn" id="chat-send-btn" title="${t('chat.send')}" aria-label="${t('chat.send')}">
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
          <line x1="12" y1="19" x2="12" y2="5"/><polyline points="5 12 12 5 19 12"/>
        </svg>
      </button>
    </div>
    <div style="display:flex;gap:8px;margin-top:8px;align-items:center;">
      <select class="input select" id="chat-model" style="width:auto;padding:4px 32px 4px 10px;font-size:0.78rem;" aria-label="模型选择">
        <option value="deepseek-chat">⚡ ${t('chat.model_flash')}</option>
        <option value="deepseek-reasoner">🧠 ${t('chat.model_pro')}</option>
      </select>
    </div>`;

  container.appendChild(messagesDiv);
  container.appendChild(inputArea);

  // Load session messages if id provided
  let session = null;
  let messages = [];
  if (id) {
    try {
      const data = await api.sessions.get(id);
      session = data;
      const msgs = await api.sessions.messages(id);
      messages = msgs;
    } catch (e) {
      showToast(t('chat.load_session_failed') + ': ' + e.message, 'error');
    }
  }

  // Render actual messages (replaces skeleton or default empty state)
  renderMessages(messagesDiv, messages);

  // ── Export button (only when a session is loaded) ──
  if (id) {
    const exportRow = document.createElement('div');
    exportRow.style.cssText = 'display:flex;gap:6px;align-items:center;margin-top:8px;';
    exportRow.innerHTML = `
      <span style="font-size:0.75rem;color:var(--text-tertiary);">${t('chat.export_label')}</span>
      <button class="btn btn-ghost btn-sm chat-export-btn" data-format="json" style="font-size:0.75rem;padding:2px 8px;">JSON</button>
      <button class="btn btn-ghost btn-sm chat-export-btn" data-format="markdown" style="font-size:0.75rem;padding:2px 8px;">MD</button>
      <button class="btn btn-ghost btn-sm chat-export-btn" data-format="html" style="font-size:0.75rem;padding:2px 8px;">HTML</button>`;
    inputArea.appendChild(exportRow);

    exportRow.querySelectorAll('.chat-export-btn').forEach(btn => {
      btn.addEventListener('click', () => {
        const format = btn.dataset.format;
        api.export.session(id, format).catch(e => showToast(t('chat.export_failed') + ': ' + e.message, 'error'));
      });
    });
  }

  // ── Send message handler ──
  const sendMessage = async () => {
    const textarea = container.querySelector('#chat-input');
    const message = textarea.value.trim();
    if (!message) return;

    // Disable input
    textarea.value = '';
    textarea.disabled = true;
    const sendBtn = container.querySelector('#chat-send-btn');
    sendBtn.disabled = true;

    // Add user message bubble
    const userBubble = createMessageBubble({
      role: 'user',
      content: message,
      created_at: new Date().toISOString(),
    });
    // Remove empty state if present
    const emptyState = messagesDiv.querySelector('.empty-state');
    if (emptyState) emptyState.remove();
    messagesDiv.appendChild(userBubble);
    smoothScrollToBottom(messagesDiv);

    // Add typing indicator
    const typingDiv = document.createElement('div');
    typingDiv.className = 'message message-assistant';
    typingDiv.innerHTML = `
      <div class="message-avatar">AI</div>
      <div class="message-bubble">
        <span class="typing-indicator">
          <span class="typing-dot"></span>
          <span class="typing-dot"></span>
          <span class="typing-dot"></span>
        </span>
      </div>`;
    messagesDiv.appendChild(typingDiv);
    smoothScrollToBottom(messagesDiv);

    try {
      const model = container.querySelector('#chat-model').value;
      const data = await api.chat.send({
        session_id: id || null,
        message,
        model,
      });

      // Remove typing indicator
      typingDiv.remove();

      // Add assistant response
      const assistantBubble = createMessageBubble({
        role: 'assistant',
        content: data.message,
        created_at: new Date().toISOString(),
      });
      messagesDiv.appendChild(assistantBubble);
      smoothScrollToBottom(messagesDiv);

      // If new session, update URL
      if (!id && data.session_id) {
        window.location.hash = `#/chat/${data.session_id}`;
        id = data.session_id;
      }
    } catch (e) {
      typingDiv.remove();
      showToast(t('chat.send_failed') + ': ' + e.message, 'error');
    } finally {
      textarea.disabled = false;
      sendBtn.disabled = false;
      textarea.style.height = 'auto';
      textarea.focus();
    }
  };

  // Event listeners
  const textarea = container.querySelector('#chat-input');
  textarea.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      sendMessage();
    }
  });
  container.querySelector('#chat-send-btn').addEventListener('click', sendMessage);

  // Auto-resize textarea
  textarea.addEventListener('input', () => {
    textarea.style.height = 'auto';
    textarea.style.height = Math.min(textarea.scrollHeight, 160) + 'px';
  });

  // Keyboard shortcuts within chat page
  container.addEventListener('keydown', (e) => {
    const tag = document.activeElement?.tagName;
    const isInput = tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT';
    if (isInput) return; // Don't intercept when typing

    if (e.ctrlKey && (e.key === 'n' || e.key === 'N')) {
      e.preventDefault();
      window.location.hash = '#/';
    }
  });

  // Auto-scroll to bottom
  requestAnimationFrame(() => { smoothScrollToBottom(messagesDiv); });

  // ── Scroll-to-bottom FAB ──
  const scrollBottomBtn = document.createElement('button');
  scrollBottomBtn.className = 'chat-scroll-bottom-btn';
  scrollBottomBtn.style.display = 'none';
  scrollBottomBtn.setAttribute('aria-label', '滚动到底部');
  scrollBottomBtn.innerHTML = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="6 9 12 15 18 9"/></svg>`;
  scrollBottomBtn.addEventListener('click', () => {
    messagesDiv.scrollTo({ top: messagesDiv.scrollHeight, behavior: 'smooth' });
  });
  messagesDiv.appendChild(scrollBottomBtn);

  const toggleScrollBtn = () => {
    const distFromBottom = messagesDiv.scrollHeight - messagesDiv.scrollTop - messagesDiv.clientHeight;
    if (distFromBottom > 150) {
      scrollBottomBtn.style.display = 'flex';
    } else {
      scrollBottomBtn.style.display = 'none';
    }
  };
  messagesDiv.addEventListener('scroll', toggleScrollBtn);

  // Focus input
  setTimeout(() => {
    const ta = container.querySelector('#chat-input');
    if (ta) ta.focus();
  }, 150);

  return container;
}

// ── Smooth auto-scroll ──
function smoothScrollToBottom(container) {
  const threshold = 100;
  const isNearBottom = container.scrollHeight - container.scrollTop - container.clientHeight < threshold;
  if (isNearBottom) {
    container.scrollTo({ top: container.scrollHeight, behavior: 'smooth' });
  }
}

function createMessageBubble(msg) {
  const div = document.createElement('div');
  const isUser = msg.role === 'user';
  div.className = `message ${isUser ? 'message-user' : 'message-assistant'}`;

  const avatar = isUser ? 'U' : 'AI';
  const renderedContent = renderMarkdown(msg.content, isUser);

  div.innerHTML = `
    <div class="message-avatar">${avatar}</div>
    <div>
      <div class="message-bubble">${renderedContent}</div>
      <div class="message-time">${formatDate(msg.created_at)}</div>
    </div>`;

  // Attach copy handlers to code blocks
  div.querySelectorAll('.copy-btn').forEach(btn => {
    btn.addEventListener('click', (e) => {
      e.stopPropagation();
      const codeBlock = btn.closest('pre');
      if (!codeBlock) return;
      const code = codeBlock.querySelector('code');
      if (!code) return;
      const text = code.textContent;
      navigator.clipboard.writeText(text).then(() => {
        btn.textContent = t('chat.copied');
        btn.classList.add('copied');
        setTimeout(() => {
          btn.textContent = t('chat.copy');
          btn.classList.remove('copied');
        }, 2000);
      }).catch(() => {
        const ta = document.createElement('textarea');
        ta.value = text;
        ta.style.cssText = 'position:fixed;left:-9999px;';
        document.body.appendChild(ta);
        ta.select();
        document.execCommand('copy');
        ta.remove();
        btn.textContent = t('chat.copied');
        btn.classList.add('copied');
        setTimeout(() => {
          btn.textContent = t('chat.copy');
          btn.classList.remove('copied');
        }, 2000);
      });
    });
  });

  return div;
}

// ── Markdown-style rendering ──
function renderMarkdown(text, isUser) {
  if (!text) return '';
  if (isUser) {
    return escapeHtml(text).replace(/\n/g, '<br>');
  }

  const codeBlocks = [];
  let processed = text.replace(/```(\w*)\n?([\s\S]*?)```/g, (_, lang, code) => {
    const idx = codeBlocks.length;
    codeBlocks.push({ lang: lang || '', code: code.replace(/\n$/, '') });
    return `%%CODEBLOCK_${idx}%%`;
  });

  processed = escapeHtml(processed);

  // Inline formatting
  processed = processed
    .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
    .replace(/\*(.+?)\*/g, '<em>$1</em>')
    .replace(/`([^`]+)`/g, '<code>$1</code>');

  // Re-insert code blocks
  processed = processed.replace(/%%CODEBLOCK_(\d+)%%/g, (_, idx) => {
    const cb = codeBlocks[parseInt(idx)];
    const langLabel = cb.lang ? `<span>${escapeHtml(cb.lang)}</span>` : '';
    return `
      <pre><div class="code-block-header">${langLabel}<button class="copy-btn">复制</button></div><code>${escapeHtml(cb.code)}</code></pre>`;
  });

  const blockTags = /(<pre[\s\S]*?<\/pre>|<ul[\s\S]*?<\/ul>|<ol[\s\S]*?<\/ol>)/g;
  const segments = processed.split(blockTags);
  processed = segments.map((seg, i) => {
    if (i % 2 === 0) {
      return seg
        .split(/\n\n+/)
        .map(p => p.trim() ? `<p>${p.replace(/\n/g, '<br>')}</p>` : '')
        .join('');
    }
    return seg;
  }).join('');

  return processed;
}

function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}
