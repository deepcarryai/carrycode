export const en = {
  input: {
    placeholder: "Type your message...",
  },
  status: {
    thinking: "Thinking",
    answering: "Answering",
    processing: "Processing",
    session_restoring: "Restoring session",
    explored: "Explored",
    edited: "Edited",
    todo: "Todo",
    bash: "Bash",
    error: "Error",
    exploring: "Exploring",
    editing: "Editing",
    idle: "Idle",
    submitting: "Submitting",
  },
  session: {
    summary_title: "Session summary",
    id: "Session ID",
    message_count: "Message count",
    description: "Description",
    no_description: "N/A",
    restore_failed: "Failed to load session history",
    loading: "Loading",
  },
  commands: {
    model: "choose what model and reasoning effort to use",
    approvals: "choose what Codex can do without approval",
    experimental: "toggle beta features",
    skills: "use skills to improve how Codex performs specific tasks",
    review: "review my current changes and find issues",
    new: "start a new chat during a conversation",
    resume: "resume a saved chat",
    session: "manage sessions",
    config: "configure system settings",
    exit: "exit the application",
    init: "create an AGENTS.md file with instructions for Codex",
    "gpt-4o": "OpenAI GPT-4o",
    "claude-3-5-sonnet": "Anthropic Claude 3.5 Sonnet",
    "gemini-pro": "Google Gemini Pro",
    session_new: "Start a new session",
    session_switch: "Switch to session",
  },
  welcome: {
    tips: [
        "Run /help to view the relevant help!",
        "Run /exit or 'Ctrl + C' to exit!"
    ]
  },
  agent_mode: {
    plan: "Plan",
    build: "Build",
  },
  confirm: {
    yes_execute: "Yes, execute",
    yes_session: "Yes, don't ask again (session)",
    no_differently: "No, tell CarryCode differently",
    tool: "Tool:",
    target: "Target:",
  },
  config: {
    title: "System Configuration",
    select_model: "Select Model",
    exit: "Exit",
    select_model_prompt: "Select a model:",
    no_models: "No models available",
    error_loading: "Error loading models:",
    switched_model: "Switched to model:",
    error_switching: "Error switching model:",
  }
};
