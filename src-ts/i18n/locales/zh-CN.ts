export const zhCN = {
  input: {
    placeholder: "请输入您的消息...",
  },
  status: {
    thinking: "思考中",
    answering: "回答中",
    processing: "处理中",
    session_restoring: "会话恢复中",
    explored: "已探索",
    edited: "已编辑",
    todo: "待办",
    bash: "执行命令",
    error: "错误",
    exploring: "探索中",
    editing: "编辑中",
    idle: "空闲",
    submitting: "提交中",
  },
  session: {
    summary_title: "会话概要",
    id: "SessionID",
    message_count: "消息记录数",
    description: "会话描述",
    no_description: "暂无",
    restore_failed: "会话历史加载失败",
    loading: "加载中",
  },
  commands: {
    model: "选择模型和推理能力",
    approvals: "设置 Codex 的免审批权限",
    experimental: "切换 Beta 功能",
    skills: "使用技能提升特定任务表现",
    review: "审查当前变更并查找问题",
    new: "在会话中开启新聊天",
    resume: "恢复已保存的聊天",
    session: "管理会话",
    config: "配置系统设置",
    exit: "退出程序",
    init: "创建包含 Codex 指令的 AGENTS.md 文件",
    "gpt-4o": "OpenAI GPT-4o",
    "claude-3-5-sonnet": "Anthropic Claude 3.5 Sonnet",
    "gemini-pro": "Google Gemini Pro",
    session_new: "开始新会话",
    session_switch: "切换会话",
  },
  welcome: {
    tips: [
        "运行 /help 查看相关帮助",
        "运行 /exit 或按 'Ctrl + C' 退出"
    ]
  },
  agent_mode: {
    plan: "Plan",
    build: "Build",
  },
  confirm: {
    yes_execute: "是，执行",
    yes_session: "是，本会话不再询问",
    no_differently: "不，告诉 CarryCode 换种方式",
    tool: "工具:",
    target: "目标:",
  },
  config: {
    title: "系统配置",
    select_model: "选择模型",
    exit: "退出",
    select_model_prompt: "选择一个模型:",
    no_models: "无可用模型",
    error_loading: "加载模型错误:",
    switched_model: "已切换到模型:",
    error_switching: "切换模型错误:",
  }
};
