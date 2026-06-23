// ── Internationalization (i18n) ──
// Translation keys for Chinese (zh) and English (en).
// Usage: t('key') returns translated string.
// Language is detected from config.language, falls back to localStorage, then 'zh'.

const TRANSLATIONS = {
  zh: {
    // ── Navigation ──
    'nav.chat': '对话',
    'nav.sessions': '会话',
    'nav.workflows': '工作流',
    'nav.tasks': '定时任务',
    'nav.settings': '设置',

    // ── Chat ──
    'chat.start_conversation': '开始对话',
    'chat.type_message': '输入消息... (Enter 发送，Shift+Enter 换行)',
    'chat.send': '发送消息',
    'chat.new_chat': '新对话',
    'chat.model_flash': 'Flash (快速)',
    'chat.model_pro': 'Pro (推理)',
    'chat.export_label': '导出:',
    'chat.copy': '复制',
    'chat.copied': '已复制',
    'chat.start_hint': '在下方输入消息，与 AI Agent 开始对话',
    'chat.send_failed': '发送失败',
    'chat.load_session_failed': '加载会话失败',
    'chat.export_failed': '导出失败',

    // ── Sessions ──
    'sessions.title': '会话',
    'sessions.new_session': '+ 新建',
    'sessions.select_session': '选择一个会话',
    'sessions.select_hint': '从左侧列表选择会话查看详情，或创建新会话',
    'sessions.no_sessions': '暂无会话',
    'sessions.no_sessions_hint': '创建第一个对话开始使用',
    'sessions.delete_confirm': '确认删除这个会话？',
    'sessions.deleted': '会话已删除',
    'sessions.delete_failed': '删除失败',
    'sessions.created': '会话已创建',
    'sessions.create_failed': '创建失败',
    'sessions.load_failed': '加载失败',
    'sessions.load_detail_failed': '加载失败',
    'sessions.no_messages': '暂无消息',
    'sessions.continue_chat': '继续对话',
    'sessions.export': '导出',
    'sessions.export_json': 'JSON 格式',
    'sessions.export_md': 'Markdown 格式',
    'sessions.export_html': 'HTML 页面',
    'sessions.session_name_prompt': '会话名称:',
    'sessions.default_session_name': '新对话',

    // ── Workflows ──
    'workflows.title': '工作流',
    'workflows.new_workflow': '新建工作流',
    'workflows.run': '运行',
    'workflows.templates': '模板',

    // ── Tasks ──
    'tasks.title': '定时任务',
    'tasks.new_task': '新建任务',
    'tasks.cron': 'Cron 表达式',
    'tasks.trigger': '触发',

    // ── Settings ──
    'settings.title': '设置',
    'settings.subtitle': '配置 LLM、渠道和系统参数',
    'settings.llm_config': 'LLM 配置',
    'settings.api_key': 'API Key',
    'settings.api_key_hint': 'DeepSeek API 密钥。从 platform.deepseek.com 获取',
    'settings.base_url': 'API Base URL',
    'settings.flash_model': 'Flash 模型 (快速响应)',
    'settings.pro_model': 'Pro 模型 (推理增强)',
    'settings.system_prompt': '系统提示词',
    'settings.temperature': '温度',
    'settings.max_tokens': '最大 Tokens',
    'settings.save_llm': '保存 LLM 配置',
    'settings.channels': '渠道接入',
    'settings.appearance': '外观',
    'settings.theme': '主题',
    'settings.theme_dark': '深色 (Dark)',
    'settings.theme_light': '亮色 (Light)',
    'settings.save_appearance': '保存外观',
    'settings.system': '系统',
    'settings.public_url': '公网地址',
    'settings.public_url_hint': '用于生成飞书回调地址、分享链接等。留空则自动使用请求来源。',
    'settings.language': '语言',
    'settings.language_zh': '中文',
    'settings.language_en': 'English',
    'settings.save_system': '保存系统配置',
    'settings.saving': '保存中...',
    'settings.saved': '配置已保存',
    'settings.save_failed': '保存失败',
    'settings.config_load_failed': '加载配置失败',

    // ── Channel Settings ──
    'channels.feishu_label': '飞书 (Feishu)',
    'channels.feishu_hint': '配置飞书自建应用的凭证信息。创建应用后，在「凭证与基础信息」页面获取。',
    'channels.app_id': 'App ID',
    'channels.app_secret': 'App Secret',
    'channels.verification_token': 'Verification Token',
    'channels.callback_label': '回调地址（填入飞书应用的事件订阅）',
    'channels.save_feishu': '保存飞书配置',
    'channels.feishu_saved': '飞书机器人 配置已保存',
    'channels.test_connection': '测试连接',
    'channels.testing': '测试中...',
    'channels.test_success': '渠道连接测试成功',
    'channels.test_failed': '渠道连接测试失败',
    'channels.qq_label': 'QQ Bot',
    'channels.qq_hint': '使用 QQ 官方机器人 API 接入。在 QQ 开放平台 创建机器人后获取凭证。',
    'channels.qq_app_id_hint': '在 QQ 开放平台「我的机器人」页面获取。',
    'channels.qq_client_secret': 'Client Secret（凭证密钥）',
    'channels.qq_client_secret_hint': '在 QQ 开放平台「开发设置」中的 Client Secret / App Secret。',
    'channels.qq_bot_secret': 'Bot Secret（Webhook 密钥，可选）',
    'channels.qq_bot_secret_hint': '用于 Webhook 事件签名验证（Ed25519）。WebSocket 模式可不填。',
    'channels.qq_ws_label': 'WebSocket 回调地址',
    'channels.qq_ws_hint': 'QQ Bot 使用 WebSocket 长连接接收消息，无需配置 HTTP 回调地址。',
    'channels.save_qq': '保存 QQ 配置',
    'channels.qq_saved': 'QQ Bot 配置已保存',
    'channels.webhook_label': '通用 Webhook',
    'channels.webhook_hint': '通用 HTTP Webhook，适用于 Zapier、n8n、IFTTT 或任何能发送 POST 请求的外部服务。AI 自动处理消息并返回 JSON 响应。',
    'channels.webhook_path': 'Webhook 路径名',
    'channels.webhook_path_hint': '自定义路径标识符。最终地址为 POST /api/channels/webhook/{此处填入的路径名}',
    'channels.webhook_full_url': 'Webhook 完整地址（配置后复制到外部服务）',
    'channels.webhook_secret': '签名密钥 (Secret)',
    'channels.webhook_secret_hint': '设置后，外部服务需要在 X-Signature-256 请求头中携带 HMAC-SHA256 签名（hex 格式）。留空则跳过签名验证。',
    'channels.webhook_json_path': 'JSON 消息路径',
    'channels.webhook_json_path_hint': '用于提取消息的 JSON 路径（用点分隔）。例如：body.text 对应 ...',
    'channels.webhook_response': '响应模板',
    'channels.webhook_response_hint': 'JSON 响应格式。可用占位符：{{reply}} = AI回复内容...',
    'channels.save_webhook': '保存 Webhook 配置',
    'channels.copy_webhook_url': '复制 Webhook 地址',
    'channels.webhook_copied': 'Webhook 地址已复制到剪贴板',
    'channels.webhook_copy_failed': '复制失败，请手动复制',
    'channels.webhook_path_required': '请先填写路径名并保存配置',
    'channels.enter_path': '请输入 Webhook 路径名',
    'channels.wechat_label': '企业微信 (WeChat Work)',
    'channels.wechat_hint': '使用企业微信自建应用接入。在企业微信管理后台创建应用后获取凭证。',
    'channels.wechat_corp_id': 'Corp ID (企业ID)',
    'channels.wechat_corp_id_hint': '在企业微信管理后台「我的企业」页面底部获取。',
    'channels.wechat_corp_secret': 'Corp Secret (应用密钥)',
    'channels.wechat_corp_secret_hint': '在企业微信管理后台「应用管理」-> 应用详情页面获取。',
    'channels.wechat_agent_id': 'Agent ID (应用ID)',
    'channels.wechat_agent_id_hint': '在企业微信管理后台「应用管理」中的 AgentId。',
    'channels.wechat_token': 'Token (回调验证 Token)',
    'channels.wechat_token_hint': '接收消息时用于验证签名的 Token，可任意填写（10 位以上），需与企业微信后台设置一致。',
    'channels.wechat_aes_key': 'Encoding AES Key (消息加密密钥)',
    'channels.wechat_aes_key_hint': '用于消息体加密的密钥，在企业微信后台随机生成或手动填写（43 位），需与后台设置一致。',
    'channels.wechat_callback_label': '回调地址（填入企业微信应用的回调配置）',
    'channels.wechat_callback_hint': 'POST 用于接收消息，GET 用于 URL 验证。',
    'channels.save_wechat': '保存企业微信配置',
    'channels.wechat_saved': '企业微信 配置已保存',

    // ── Onboarding ──
    'onboarding.welcome_title': '欢迎使用 AI Agent',
    'onboarding.welcome_desc': '让我们花 2 分钟完成初始配置，然后你就可以开始使用 AI Agent 了。',
    'onboarding.welcome_items': '配置 DeepSeek API 密钥<br>连接飞书/QQ/微信<br>创建个性化 AI Agent<br>设置定时任务和工作流',
    'onboarding.llm_title': '配置 LLM',
    'onboarding.llm_desc': '填入 DeepSeek API 密钥，选择默认模型。',
    'onboarding.channels_title': '连接渠道',
    'onboarding.channels_desc': '配置飞书 Bot，这样你就可以在飞书中与 AI 对话。（可跳过）',
    'onboarding.channels_hint': '稍后可在「设置→渠道接入」中配置更多渠道',
    'onboarding.agent_title': '创建第一个 Agent',
    'onboarding.agent_desc': '给你的 AI 助手起个名字，设定它的角色。',
    'onboarding.done_title': '一切就绪！',
    'onboarding.done_desc': '配置完成，开始享受 AI Agent 带来的便利吧。',
    'onboarding.prev': '上一步',
    'onboarding.next': '下一步',
    'onboarding.skip': '跳过',
    'onboarding.start': '开始使用',
    'onboarding.step': '步骤',
    'onboarding.of': ' / ',
    'onboarding.done_text': '配置完成！你现在可以：',
    'onboarding.done_items': '在 Web 端与 AI 对话<br>在飞书中 @机器人 提问<br>创建自动化工作流<br>设置定时任务',

    // ── Common ──
    'common.loading': '加载中...',
    'common.error': '错误',
    'common.success': '成功',
    'common.cancel': '取消',
    'common.confirm': '确认',
    'common.save': '保存',
    'common.delete': '删除',
    'common.retry': '重试',

    // ── Shortcuts Modal ──
    'shortcuts.title': '键盘快捷键',
    'shortcuts.close': '关闭',
    'shortcuts.section_global': '全局',
    'shortcuts.cmd_palette': '命令面板 (快速搜索命令与会话)',
    'shortcuts.toggle_shortcuts': '显示/隐藏快捷键面板',
    'shortcuts.esc_cancel': '关闭面板 / 取消操作',
    'shortcuts.nav_tabs': '导航: 对话 / 会话 / 工作流 / 定时任务 / 设置',
    'shortcuts.back_forward': '后退 / 前进',
    'shortcuts.section_chat': '对话',
    'shortcuts.enter_send': '发送消息',
    'shortcuts.shift_enter_newline': '换行',
    'shortcuts.ctrl_enter_send': '发送消息',
    'shortcuts.ctrl_n_new': '新建对话',
    'shortcuts.section_workflow': '工作流编辑器',
    'shortcuts.ctrl_s_save': '保存工作流',
    'shortcuts.ctrl_enter_run': '运行工作流',
    'shortcuts.del_node': '删除选中节点/连线',
    'shortcuts.fit_canvas': '适应画布',
    'shortcuts.auto_layout': '自动布局',
    'shortcuts.edge_mode': '连线模式',
    'shortcuts.footer': '按 ? 随时查看快捷键，按 Ctrl+K 打开命令面板',

    // ── Command Palette ──
    'palette.placeholder': '搜索命令、会话...',
    'palette.navigate': '导航',
    'palette.select': '选择',
    'palette.close': '关闭',
    'palette.no_results': '无匹配结果',
    'palette.section_actions': '快捷操作',
    'palette.section_nav': '页面导航',
    'palette.section_other': '其他',
    'palette.section_sessions': '会话',
    'palette.section_recent': '最近会话',
    'palette.cmd_new_chat': '新建对话',
    'palette.cmd_new_chat_sub': '开始一个新的 AI 对话',
    'palette.cmd_new_workflow': '新建工作流',
    'palette.cmd_new_workflow_sub': '创建自动化工作流编排',
    'palette.cmd_new_task': '新建定时任务',
    'palette.cmd_new_task_sub': '创建定时执行的自动化任务',
    'palette.cmd_toggle_theme': '切换颜色主题',
    'palette.cmd_toggle_theme_sub': '在深色与亮色模式间切换',
    'palette.cmd_sessions': '会话管理',
    'palette.cmd_sessions_sub': '浏览与管理所有会话',
    'palette.cmd_config': '应用设置',
    'palette.cmd_config_sub': '配置模型、渠道与偏好',
    'palette.cmd_shortcuts': '键盘快捷键',
    'palette.cmd_shortcuts_sub': '查看所有可用的快捷键',
    'palette.badge_command': '命令',
    'palette.unnamed_session': '未命名会话',

    // ── Search ──
    'search.placeholder': '搜索会话或消息...',
    'search.clear': '清除搜索',
    'search.results': '找到 {total} 个结果 \"{query}\"',
    'search.no_results': '未找到 \"{query}\" 的匹配结果',
    'search.try_other': '换个关键词试试',
    'search.failed': '搜索失败',
    'search.type_session': '会话',
    'search.type_message': '消息',
    'search.relevance': '相关度',

    // ── Theme ──
    'theme.dark': '深色模式',
    'theme.light': '亮色模式',
    'theme.switch': '切换主题',

    // ── Page / Errors ──
    'page.not_found': '页面未找到',
    'page.not_found_text': '路由 \"{path}\" 不存在',
    'page.home': '返回首页',
    'page.load_failed': '页面加载失败',
    'page.loaded': '加载完成',
    'page.menu': '菜单',
    'page.shortcut_hint': '快捷键',
  },

  en: {
    // ── Navigation ──
    'nav.chat': 'Chat',
    'nav.sessions': 'Sessions',
    'nav.workflows': 'Workflows',
    'nav.tasks': 'Tasks',
    'nav.settings': 'Settings',

    // ── Chat ──
    'chat.start_conversation': 'Start a conversation',
    'chat.type_message': 'Type a message... (Enter to send, Shift+Enter for new line)',
    'chat.send': 'Send Message',
    'chat.new_chat': 'New Chat',
    'chat.model_flash': 'Flash (Fast)',
    'chat.model_pro': 'Pro (Reasoning)',
    'chat.export_label': 'Export:',
    'chat.copy': 'Copy',
    'chat.copied': 'Copied',
    'chat.start_hint': 'Type a message below to start chatting with AI Agent',
    'chat.send_failed': 'Send failed',
    'chat.load_session_failed': 'Failed to load session',
    'chat.export_failed': 'Export failed',

    // ── Sessions ──
    'sessions.title': 'Sessions',
    'sessions.new_session': '+ New',
    'sessions.select_session': 'Select a session',
    'sessions.select_hint': 'Choose a session from the list to view details, or create a new one',
    'sessions.no_sessions': 'No sessions yet',
    'sessions.no_sessions_hint': 'Create your first conversation to get started',
    'sessions.delete_confirm': 'Delete this session?',
    'sessions.deleted': 'Session deleted',
    'sessions.delete_failed': 'Delete failed',
    'sessions.created': 'Session created',
    'sessions.create_failed': 'Create failed',
    'sessions.load_failed': 'Load failed',
    'sessions.load_detail_failed': 'Load failed',
    'sessions.no_messages': 'No messages',
    'sessions.continue_chat': 'Continue Chat',
    'sessions.export': 'Export',
    'sessions.export_json': 'JSON Format',
    'sessions.export_md': 'Markdown Format',
    'sessions.export_html': 'HTML Page',
    'sessions.session_name_prompt': 'Session name:',
    'sessions.default_session_name': 'New Chat',

    // ── Workflows ──
    'workflows.title': 'Workflows',
    'workflows.new_workflow': 'New Workflow',
    'workflows.run': 'Run',
    'workflows.templates': 'Templates',

    // ── Tasks ──
    'tasks.title': 'Scheduled Tasks',
    'tasks.new_task': 'New Task',
    'tasks.cron': 'Cron Expression',
    'tasks.trigger': 'Trigger',

    // ── Settings ──
    'settings.title': 'Settings',
    'settings.subtitle': 'Configure LLM, channels and system parameters',
    'settings.llm_config': 'LLM Configuration',
    'settings.api_key': 'API Key',
    'settings.api_key_hint': 'DeepSeek API key. Get it from platform.deepseek.com',
    'settings.base_url': 'API Base URL',
    'settings.flash_model': 'Flash Model (fast response)',
    'settings.pro_model': 'Pro Model (reasoning enhanced)',
    'settings.system_prompt': 'System Prompt',
    'settings.temperature': 'Temperature',
    'settings.max_tokens': 'Max Tokens',
    'settings.save_llm': 'Save LLM Configuration',
    'settings.channels': 'Channels',
    'settings.appearance': 'Appearance',
    'settings.theme': 'Theme',
    'settings.theme_dark': 'Dark',
    'settings.theme_light': 'Light',
    'settings.save_appearance': 'Save Appearance',
    'settings.system': 'System',
    'settings.public_url': 'Public URL',
    'settings.public_url_hint': 'Used to generate Feishu callback URL, share links etc. Leave empty to auto-detect.',
    'settings.language': 'Language',
    'settings.language_zh': 'Chinese',
    'settings.language_en': 'English',
    'settings.save_system': 'Save System Configuration',
    'settings.saving': 'Saving...',
    'settings.saved': 'Configuration saved',
    'settings.save_failed': 'Save failed',
    'settings.config_load_failed': 'Failed to load configuration',

    // ── Channel Settings ──
    'channels.feishu_label': 'Feishu',
    'channels.feishu_hint': 'Configure Feishu self-built app credentials. Find them in the "Credentials & Basic Info" page after creating the app.',
    'channels.app_id': 'App ID',
    'channels.app_secret': 'App Secret',
    'channels.verification_token': 'Verification Token',
    'channels.callback_label': 'Callback URL (enter in Feishu app event subscription)',
    'channels.save_feishu': 'Save Feishu Configuration',
    'channels.feishu_saved': 'Feishu Bot configuration saved',
    'channels.test_connection': 'Test Connection',
    'channels.testing': 'Testing...',
    'channels.test_success': 'Channel connection test succeeded',
    'channels.test_failed': 'Channel connection test failed',
    'channels.qq_label': 'QQ Bot',
    'channels.qq_hint': 'Use QQ official Bot API. Create a bot on QQ Open Platform to get credentials.',
    'channels.qq_app_id_hint': 'Find on the "My Bot" page on QQ Open Platform.',
    'channels.qq_client_secret': 'Client Secret',
    'channels.qq_client_secret_hint': 'Client Secret / App Secret in "Dev Settings" on QQ Open Platform.',
    'channels.qq_bot_secret': 'Bot Secret (Webhook secret, optional)',
    'channels.qq_bot_secret_hint': 'Used for Webhook event signature verification (Ed25519). Not needed for WebSocket mode.',
    'channels.qq_ws_label': 'WebSocket Callback URL',
    'channels.qq_ws_hint': 'QQ Bot uses WebSocket persistent connection for messages, no HTTP callback needed.',
    'channels.save_qq': 'Save QQ Configuration',
    'channels.qq_saved': 'QQ Bot configuration saved',
    'channels.webhook_label': 'Generic Webhook',
    'channels.webhook_hint': 'Generic HTTP Webhook for Zapier, n8n, IFTTT, or any external service that sends POST requests. AI processes messages and returns JSON responses.',
    'channels.webhook_path': 'Webhook Path',
    'channels.webhook_path_hint': 'Custom path identifier. Final URL will be POST /api/channels/webhook/{your-path}',
    'channels.webhook_full_url': 'Full Webhook URL (copy to external service after saving)',
    'channels.webhook_secret': 'Signing Secret',
    'channels.webhook_secret_hint': 'If set, external services must include HMAC-SHA256 signature (hex) in X-Signature-256 header. Leave empty to skip verification.',
    'channels.webhook_json_path': 'JSON Message Path',
    'channels.webhook_json_path_hint': 'Dot-separated JSON path to extract the message. E.g.: body.text for {"body": {"text": "..."}}',
    'channels.webhook_response': 'Response Template',
    'channels.webhook_response_hint': 'JSON response format. Placeholders: {{reply}} = AI reply, {{timestamp}} = response time.',
    'channels.save_webhook': 'Save Webhook Configuration',
    'channels.copy_webhook_url': 'Copy Webhook URL',
    'channels.webhook_copied': 'Webhook URL copied to clipboard',
    'channels.webhook_copy_failed': 'Copy failed, please copy manually',
    'channels.webhook_path_required': 'Please fill in the path and save first',
    'channels.enter_path': 'Please enter the webhook path',
    'channels.wechat_label': 'WeChat Work',
    'channels.wechat_hint': 'Use WeChat Work self-built app. Create an app in WeChat Work Admin Console to get credentials.',
    'channels.wechat_corp_id': 'Corp ID',
    'channels.wechat_corp_id_hint': 'Find at the bottom of "My Company" page in WeChat Work Admin Console.',
    'channels.wechat_corp_secret': 'Corp Secret',
    'channels.wechat_corp_secret_hint': 'Find in the app details page under "App Management" in WeChat Work Admin Console.',
    'channels.wechat_agent_id': 'Agent ID',
    'channels.wechat_agent_id_hint': 'The AgentId found in "App Management" in WeChat Work Admin Console.',
    'channels.wechat_token': 'Token (callback verification)',
    'channels.wechat_token_hint': 'Token for signature verification when receiving messages. Can be any string (10+ chars), must match WeChat Work backend settings.',
    'channels.wechat_aes_key': 'Encoding AES Key',
    'channels.wechat_aes_key_hint': 'Key for message body encryption. Generate randomly or enter manually (43 chars) in WeChat Work backend.',
    'channels.wechat_callback_label': 'Callback URL (enter in WeChat Work app callback config)',
    'channels.wechat_callback_hint': 'POST receives messages, GET for URL verification.',
    'channels.save_wechat': 'Save WeChat Work Configuration',
    'channels.wechat_saved': 'WeChat Work configuration saved',

    // ── Onboarding ──
    'onboarding.welcome_title': 'Welcome to AI Agent',
    'onboarding.welcome_desc': 'Let\'s spend 2 minutes on initial setup, then you can start using AI Agent.',
    'onboarding.welcome_items': 'Configure DeepSeek API key<br>Connect Feishu/QQ/WeChat<br>Create a personalized AI Agent<br>Set up scheduled tasks and workflows',
    'onboarding.llm_title': 'Configure LLM',
    'onboarding.llm_desc': 'Enter your DeepSeek API key and choose a default model.',
    'onboarding.channels_title': 'Connect Channels',
    'onboarding.channels_desc': 'Configure Feishu Bot so you can chat with AI in Feishu. (Can be skipped)',
    'onboarding.channels_hint': 'You can configure more channels later in Settings > Channels',
    'onboarding.agent_title': 'Create Your First Agent',
    'onboarding.agent_desc': 'Give your AI assistant a name and define its role.',
    'onboarding.done_title': 'All Set!',
    'onboarding.done_desc': 'Configuration complete. Enjoy the convenience of AI Agent.',
    'onboarding.prev': 'Previous',
    'onboarding.next': 'Next',
    'onboarding.skip': 'Skip',
    'onboarding.start': 'Get Started',
    'onboarding.step': 'Step',
    'onboarding.of': ' of ',
    'onboarding.done_text': 'Setup complete! You can now:',
    'onboarding.done_items': 'Chat with AI on the web<br>@mention the bot in Feishu<br>Create automated workflows<br>Set up scheduled tasks',

    // ── Common ──
    'common.loading': 'Loading...',
    'common.error': 'Error',
    'common.success': 'Success',
    'common.cancel': 'Cancel',
    'common.confirm': 'Confirm',
    'common.save': 'Save',
    'common.delete': 'Delete',
    'common.retry': 'Retry',

    // ── Shortcuts Modal ──
    'shortcuts.title': 'Keyboard Shortcuts',
    'shortcuts.close': 'Close',
    'shortcuts.section_global': 'Global',
    'shortcuts.cmd_palette': 'Command Palette (quick search commands & sessions)',
    'shortcuts.toggle_shortcuts': 'Show / hide shortcuts panel',
    'shortcuts.esc_cancel': 'Close panel / cancel action',
    'shortcuts.nav_tabs': 'Navigate: Chat / Sessions / Workflows / Tasks / Settings',
    'shortcuts.back_forward': 'Back / Forward',
    'shortcuts.section_chat': 'Chat',
    'shortcuts.enter_send': 'Send message',
    'shortcuts.shift_enter_newline': 'New line',
    'shortcuts.ctrl_enter_send': 'Send message',
    'shortcuts.ctrl_n_new': 'New chat',
    'shortcuts.section_workflow': 'Workflow Editor',
    'shortcuts.ctrl_s_save': 'Save workflow',
    'shortcuts.ctrl_enter_run': 'Run workflow',
    'shortcuts.del_node': 'Delete selected node/edge',
    'shortcuts.fit_canvas': 'Fit canvas',
    'shortcuts.auto_layout': 'Auto layout',
    'shortcuts.edge_mode': 'Edge mode',
    'shortcuts.footer': 'Press ? anytime for shortcuts, Ctrl+K for command palette',

    // ── Command Palette ──
    'palette.placeholder': 'Search commands, sessions...',
    'palette.navigate': 'Navigate',
    'palette.select': 'Select',
    'palette.close': 'Close',
    'palette.no_results': 'No results found',
    'palette.section_actions': 'Quick Actions',
    'palette.section_nav': 'Navigation',
    'palette.section_other': 'Other',
    'palette.section_sessions': 'Sessions',
    'palette.section_recent': 'Recent Sessions',
    'palette.cmd_new_chat': 'New Chat',
    'palette.cmd_new_chat_sub': 'Start a new AI conversation',
    'palette.cmd_new_workflow': 'New Workflow',
    'palette.cmd_new_workflow_sub': 'Create an automated workflow pipeline',
    'palette.cmd_new_task': 'New Scheduled Task',
    'palette.cmd_new_task_sub': 'Create a scheduled automation task',
    'palette.cmd_toggle_theme': 'Toggle Color Theme',
    'palette.cmd_toggle_theme_sub': 'Switch between dark and light mode',
    'palette.cmd_sessions': 'Session Management',
    'palette.cmd_sessions_sub': 'Browse and manage all sessions',
    'palette.cmd_config': 'App Settings',
    'palette.cmd_config_sub': 'Configure models, channels & preferences',
    'palette.cmd_shortcuts': 'Keyboard Shortcuts',
    'palette.cmd_shortcuts_sub': 'View all available shortcuts',
    'palette.badge_command': 'Command',
    'palette.unnamed_session': 'Untitled Session',

    // ── Search ──
    'search.placeholder': 'Search sessions or messages...',
    'search.clear': 'Clear search',
    'search.results': 'Found {total} results for \"{query}\"',
    'search.no_results': 'No results found for \"{query}\"',
    'search.try_other': 'Try different keywords',
    'search.failed': 'Search failed',
    'search.type_session': 'Session',
    'search.type_message': 'Message',
    'search.relevance': 'Relevance',

    // ── Theme ──
    'theme.dark': 'Dark Mode',
    'theme.light': 'Light Mode',
    'theme.switch': 'Switch Theme',

    // ── Page / Errors ──
    'page.not_found': 'Page Not Found',
    'page.not_found_text': 'Route \"{path}\" does not exist',
    'page.home': 'Back to Home',
    'page.load_failed': 'Page load failed',
    'page.loaded': 'Loaded',
    'page.menu': 'Menu',
    'page.shortcut_hint': 'Shortcuts',
  },
};

// ── State ──
let currentLang = (function () {
  try {
    const stored = localStorage.getItem('ai-agent-lang');
    if (stored && TRANSLATIONS[stored]) return stored;
  } catch (_) { /* localStorage unavailable */ }
  return 'zh';
})();

// ── Translation function ──
// t('key') returns the translated string for the current language.
// Falls back to English, then returns the key itself if not found.
function t(key, vars) {
  const langPack = TRANSLATIONS[currentLang] || TRANSLATIONS['en'] || {};
  let result = langPack[key];
  if (result === undefined) {
    result = (TRANSLATIONS['en'] || {})[key];
  }
  if (result === undefined) {
    return key;
  }
  // Variable substitution: t('key', { total: 5, query: 'hello' })
  if (vars) {
    Object.keys(vars).forEach(k => {
      result = result.replace(new RegExp('\\{' + k + '\\}', 'g'), vars[k]);
    });
  }
  return result;
}

// ── Set language ──
function setLanguage(lang) {
  if (TRANSLATIONS[lang]) {
    currentLang = lang;
    try { localStorage.setItem('ai-agent-lang', lang); } catch (_) {}
    applyStaticTranslations();
  }
  return currentLang;
}

// ── Get current language ──
function getLanguage() {
  return currentLang;
}

// ── Apply translations to static DOM elements ──
// Scans elements with data-i18n, data-i18n-placeholder, data-i18n-aria, data-i18n-title
function applyStaticTranslations() {
  // data-i18n: sets textContent
  document.querySelectorAll('[data-i18n]').forEach(el => {
    const key = el.dataset.i18n;
    if (key) {
      el.textContent = t(key);
    }
  });

  // data-i18n-placeholder: sets placeholder attribute
  document.querySelectorAll('[data-i18n-placeholder]').forEach(el => {
    const key = el.dataset.i18nPlaceholder;
    if (key) {
      el.placeholder = t(key);
    }
  });

  // data-i18n-aria: sets aria-label attribute
  document.querySelectorAll('[data-i18n-aria]').forEach(el => {
    const key = el.dataset.i18nAria;
    if (key) {
      el.setAttribute('aria-label', t(key));
    }
  });

  // data-i18n-title: sets title attribute
  document.querySelectorAll('[data-i18n-title]').forEach(el => {
    const key = el.dataset.i18nTitle;
    if (key) {
      el.setAttribute('title', t(key));
    }
  });

  // data-i18n-html: sets innerHTML (use sparingly)
  document.querySelectorAll('[data-i18n-html]').forEach(el => {
    const key = el.dataset.i18nHtml;
    if (key) {
      el.innerHTML = t(key);
    }
  });
}

// ── Expose globally (for inline usage in HTML and other non-module contexts) ──
window.t = t;
window.setLanguage = setLanguage;
window.getLanguage = getLanguage;

// ── Module exports ──
export { t, setLanguage, getLanguage, applyStaticTranslations, TRANSLATIONS, currentLang };

// ── Apply translations on load (for saved language preference) ──
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', applyStaticTranslations);
} else {
  applyStaticTranslations();
}
