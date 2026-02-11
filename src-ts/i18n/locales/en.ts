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
    theme: "select UI theme",
    language: "select interface language",
  approval: "choose what CarryCode can do without approval",
    experimental: "toggle beta features",
  skill: "use skills to improve how CarryCode perform specific tasks",
    review: "review my current changes and find issues",
    new: "start a new chat during a conversation",
    session: "manage sessions",
    exit: "exit the application",
  init: "create an AGENTS.md file with instructions for CarryCode",
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
  welcome_wizard: {
    list_separator: ", ",
    titles: {
      language: "Choose language",
      theme: "Choose theme",
      agent: "Choose agent mode",
      provider_brand: "Select an LLM provider",
      provider_details: "Fill provider details",
      review: "Review",
    },
    progress: {
      language: "Language",
      theme: "Theme",
      agent: "Agent",
      provider: "Provider",
      review: "Review",
    },
    hints: {
      list: "↑/↓ to select, Enter to confirm, Esc to go back",
      provider: "Tab switches fields, Shift+Tab goes back, Enter goes next, Esc to go back",
      review: "Press Enter to save, Esc to go back",
    },
    labels: {
      next: "next",
      language: "Language",
      theme: "Theme",
      agent: "Agent",
      provider: "Provider",
    },
    errors: {
      fill_first: "Please fill: {{fields}}",
      provider_incomplete: "Provider info is incomplete",
      base_url_invalid: "base_url must be an http(s) URL",
    },
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
  }
};
