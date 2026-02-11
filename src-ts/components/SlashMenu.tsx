import React from 'react';
import { useTranslation } from 'react-i18next';
import { Box, Text } from 'ink';
import { useTheme } from '../theme/index.js';
import type { Key } from '../hooks/useKeypress.js';
import type { TextBuffer } from '../hooks/useTextBuffer.js';
import type { AvailableModel } from '../types/index.js';
import type { SavedSessionInfo } from '../hooks/useRustBridge.js';
import i18n from '../i18n/index.js';

export interface SlashCommand {
  name: string;
  description: string;
  subCommands?: SlashCommand[];
  isActive?: boolean;
}

export const SLASH_COMMANDS: SlashCommand[] = [
  {
    name: 'approval',
    description: 'Select Approval Mode',
    subCommands: [
      { name: 'read-only', description: 'Requires approval to edit files and run commands.' },
      { name: 'agent', description: 'Read and edit files, and run commands.' },
      { name: 'agent-full', description: 'Edit outside workspace and run network commands. Use with caution.' },
    ],
  },
  {
    name: 'language',
    description: 'select interface language',
    subCommands: [],
  },
  {
    name: 'mcp',
    description: 'manage mcp servers',
    subCommands: [], // Populated dynamically
  },
  {
    name: 'model',
    description: 'select LLM model',
    subCommands: [],
  },
  {
    name: 'session',
    description: 'manage sessions',
    subCommands: [
      { name: 'New', description: 'Start a new session' },
      // Other sessions populated dynamically
    ],
  },
  {
    name: 'skill',
    description: 'use skills to improve how Codex performs specific tasks',
    subCommands: [
      { name: 'list', description: 'List available skills' },
      { name: 'enable', description: 'Enable a skill' },
      { name: 'disable', description: 'Disable a skill' },
    ],
  },
  {
    name: 'theme',
    description: 'select UI theme',
    subCommands: [],
  },
  {
    name: 'exit',
    description: 'exit the application',
  }
];

export interface SlashMenuState {
  isOpen: boolean;
  options: SlashCommand[];
  level: number;
  prefix: string;
}

export function useSlashMenuState(
  text: string,
  availableModels: any[],
  availableThemes: any[],
  sessions: SavedSessionInfo[],
  mcpServers: Record<string, any>,
  availableSkills: any[],
  currentThemeName?: string,
  currentModelName?: string,
  currentApprovalMode?: string,
  currentLanguage?: string
): SlashMenuState {
  return React.useMemo(() => {
    if (!text.startsWith('/')) {
      return { isOpen: false, options: [], level: 0, prefix: '' };
    }

    if (text.startsWith('/skill ')) {
      const sub = text.replace('/skill ', '');
      if (sub.startsWith('enable ') || sub.startsWith('disable ')) {
        const action = sub.startsWith('enable ') ? 'enable' : 'disable';
        const query = sub.replace(action + ' ', '').toLowerCase();
        const options = availableSkills
          .map((s) => ({
            name: s.name,
            description: s.description || 'skill',
          }))
          .filter((o) => o.name.toLowerCase().startsWith(query));
        return { isOpen: true, options, level: 3, prefix: `/skill ${action} ` };
      }
    }

    if (text === '/mcp') {
      const dynamicServers = Object.entries(mcpServers).map(([name, cfg]) => ({
        name,
        description:
          cfg && typeof cfg.url === 'string'
            ? cfg.url
            : cfg && typeof cfg.command === 'string'
              ? `stdio: ${cfg.command}`
              : 'mcp server',
      }));
      return { isOpen: true, options: dynamicServers, level: 2, prefix: `/mcp ` };
    }

    // Parse command: /cmd subcmd ...
    // Regex matches: /cmd (group1) remaining (group2)
    // or /cmd

    // Check level 2 (e.g. /model gpt-4o-mini)
    const matchL2 = text.match(/^\/([a-zA-Z0-9_-]+)\s+(.*)$/);
    if (matchL2) {
      const cmdName = matchL2[1];
      const subQuery = matchL2[2] || '';

      if (cmdName === 'model') {
        const q = subQuery.toLowerCase();
        const options = availableModels
          .map((m) => ({
            name: m.model,
            description: `${m.provider} ${m.model}`,
            isActive: currentModelName ? m.model.toLowerCase() === currentModelName.toLowerCase() : false,
          }))
          .filter((o) => o.name.toLowerCase().startsWith(q));

        const merged: SlashCommand[] = [];
        if ('add'.startsWith(q)) {
          merged.push({ name: 'add', description: 'add provider/model' });
        }
        merged.push(...options);

        return { isOpen: merged.length > 0, options: merged, level: 2, prefix: `/model ` };
      }

      if (cmdName === 'theme') {
        const q = subQuery.toLowerCase();
        const options = availableThemes
          .map((t) => ({
            name: t.name,
            description: t.name,
            isActive: currentThemeName ? t.name === currentThemeName : false,
          }))
          .filter((o) => o.name.toLowerCase().startsWith(q));
        return { isOpen: options.length > 0, options, level: 2, prefix: `/theme ` };
      }

      if (cmdName === 'language') {
        const q = subQuery.toLowerCase();
        const availableLanguages = [
          { name: 'en', description: 'English' },
          { name: 'zh-CN', description: '简体中文' },
        ];
        const options = availableLanguages
          .map((lang) => ({
            name: lang.name,
            description: lang.description,
            isActive: currentLanguage ? lang.name === currentLanguage : false,
          }))
          .filter((o) => o.name.toLowerCase().startsWith(q));
        return { isOpen: options.length > 0, options, level: 2, prefix: `/language ` };
      }

      const cmd = SLASH_COMMANDS.find((c) => c.name === cmdName);
      if (cmd) {
        if (cmdName === 'session') {
          // Dynamic session list
          const dynamicSessions = [
            { name: 'New', description: 'session_new' },
            ...sessions.map(s => {
              const date = new Date(s.updatedAtMs);
              const timeStr = date.toLocaleString();
              const title = s.title || 'Untitled';
              return { 
                name: s.sessionId, 
                description: `${title} (${timeStr})`
              };
            })
          ];
          const filtered = dynamicSessions.filter(s => s.name.toLowerCase().startsWith(subQuery.toLowerCase()));
          return { isOpen: true, options: filtered, level: 2, prefix: `/session ` };
        }

        if (cmdName === 'mcp') {
          // Dynamic mcp servers list
          const dynamicServers = Object.entries(mcpServers).map(([name, cfg]) => ({
            name,
            description:
              cfg && typeof cfg.url === 'string'
                ? cfg.url
                : cfg && typeof cfg.command === 'string'
                  ? `stdio: ${cfg.command}`
                  : 'mcp server',
          }));
          const filtered = dynamicServers.filter(s => s.name.toLowerCase().startsWith(subQuery.toLowerCase()));
          return { isOpen: true, options: filtered, level: 2, prefix: `/mcp ` };
        }

        if (cmd.subCommands) {
          const subCommands =
            cmdName === 'approval'
              ? cmd.subCommands.map((sc) => ({
                ...sc,
                isActive: currentApprovalMode ? sc.name === currentApprovalMode : false,
              }))
              : cmd.subCommands;
          const filtered = subCommands.filter((sc) => sc.name.toLowerCase().startsWith(subQuery.toLowerCase()));
          return { isOpen: true, options: filtered, level: 2, prefix: `/${cmdName} ` };
        }
      }
      return { isOpen: false, options: [], level: 0, prefix: '' };
    }

    // Check level 1 (e.g. /model)
    const query = text.slice(1);
    if (query.includes(' ')) return { isOpen: false, options: [], level: 0, prefix: '' };

    const filtered = SLASH_COMMANDS.filter((c) => c.name.toLowerCase().startsWith(query.toLowerCase()));
    return {
      isOpen: filtered.length > 0,
      options: filtered,
      level: 1,
      prefix: '/',
    };
  }, [text, availableModels, availableThemes, sessions, mcpServers, availableSkills, currentThemeName, currentModelName, currentApprovalMode, currentLanguage]);
}

export interface SlashMenuAction {
  type: 'execute' | 'autocomplete';
  payload: string;
  isModel?: boolean;
  isTheme?: boolean;
  isLanguage?: boolean;
  isSession?: boolean;
  isApproval?: boolean;
  isExit?: boolean;
  isSkill?: boolean;
}

export function getSlashMenuAction(
  menuState: SlashMenuState,
  selected: SlashCommand
): SlashMenuAction {
  const newValue = `${menuState.prefix}${selected.name}`;

  const isModel = menuState.prefix === '/model ';
  const isTheme = menuState.prefix === '/theme ';
  const isLanguage = menuState.prefix === '/language ';
  const isSession = menuState.prefix === '/session ';
  const isApproval = menuState.prefix === '/approval ';
  const isExit = menuState.prefix === '/' && selected.name === 'exit';
  const isSkill = menuState.prefix.startsWith('/skill ');

  if (isModel || isTheme || isLanguage || isSession || isApproval || isExit || isSkill) {
    return {
      type: 'execute',
      payload: newValue,
      isModel,
      isTheme,
      isLanguage,
      isSession,
      isApproval,
      isExit,
      isSkill
    };
  }

  if ((selected as any).subCommands) {
    return {
      type: 'autocomplete',
      payload: newValue + ' '
    };
  }

  return {
    type: 'autocomplete',
    payload: newValue
  };
}

export function isSlashMenuNavigationKey(key: Key): boolean {
  return (
    key.name === 'up' ||
    key.name === 'down' ||
    key.name === 'return' ||
    key.name === 'tab' ||
    key.name === 'escape'
  );
}

export interface SlashMenuHandlersDeps {
  menuState: SlashMenuState;
  buffer: TextBuffer;
  availableModels: AvailableModel[];
  availableThemes: Array<{ name: string }>;
  sessionId: string;
  setModel: (sessionId: string, provider: string, model: string) => Promise<void>;
  setThemeName: (name: string) => void;
  setLanguage: (language: string) => Promise<void>;
  setApprovalMode: (mode: 'read-only' | 'agent' | 'agent-full') => Promise<void>;
  setStatusMessage: (message: string) => void;
  setCurrentModelName: (name: string) => void;
  openModelWizard: () => void;
  onSubmit: (text: string) => void;
  exit: () => void;
}

export function createSlashMenuHandlers({
  menuState,
  buffer,
  availableModels,
  sessionId,
  setModel,
  setThemeName,
  setLanguage,
  setApprovalMode,
  setStatusMessage,
  setCurrentModelName,
  openModelWizard,
  onSubmit,
  exit,
}: SlashMenuHandlersDeps): {
  handleMenuSelect: (selected: SlashCommand) => void;
  handleMenuEscape: () => void;
} {
  const handleMenuSelect = (selected: SlashCommand) => {
    const action = getSlashMenuAction(menuState, selected);

    if (action.type === 'execute') {
      if (action.isModel) {
        if (selected.name === 'add') {
          openModelWizard();
          buffer.setText('');
          return;
        }
        const model = availableModels.find((m) => m.model === selected.name);
        if (model) {
          setModel(sessionId, model.provider, model.model)
            .then(() => {
              setStatusMessage(`Switched model to ${model.model}`);
              setCurrentModelName(model.model);
            })
            .catch((e) => setStatusMessage(`Failed to switch model: ${e}`));
          setTimeout(() => setStatusMessage(''), 3000);
        }
      } else if (action.isTheme) {
        setThemeName(selected.name);
        setStatusMessage(`Switched theme to ${selected.name}`);
        setTimeout(() => setStatusMessage(''), 3000);
      } else if (action.isLanguage) {
        setLanguage(selected.name)
          .then(() => {
            // Immediately switch the UI language
            i18n.changeLanguage(selected.name);
            setStatusMessage(`Switched language to ${selected.name}`);
          })
          .catch((e) => setStatusMessage(`Failed to switch language: ${e}`));
        setTimeout(() => setStatusMessage(''), 3000);
      } else if (action.isApproval) {
        const mode = selected.name as 'read-only' | 'agent' | 'agent-full';
        setApprovalMode(mode)
          .then(() => {
            setStatusMessage(`Switched approval mode to ${mode}`);
          })
          .catch((e) => setStatusMessage(`Failed to switch approval mode: ${e}`));
        setTimeout(() => setStatusMessage(''), 3000);
      } else if (action.isSession || action.isSkill) {
        onSubmit(action.payload);
      } else if (action.isExit) {
        buffer.setText('');
        setTimeout(() => exit(), 10);
        return;
      }
      buffer.setText('');
      return;
    }

    if (buffer.text === action.payload && !action.payload.endsWith(' ')) {
      onSubmit(action.payload);
      buffer.setText('');
      return;
    }
    buffer.setText(action.payload);
  };

  const handleMenuEscape = () => {
    if (!menuState.isOpen) return;

    const text = buffer.text;
    const matchL2 = text.match(/^\/([a-zA-Z0-9_-]+)\s+/);
    if (matchL2) {
      buffer.setText('/');
      return;
    }

    buffer.setText('');
  };

  return { handleMenuSelect, handleMenuEscape };
}

export interface SlashCommandSubmitDeps {
  text: string;
  buffer: TextBuffer;
  availableModels: AvailableModel[];
  availableThemes: Array<{ name: string }>;
  sessionId: string;
  setModel: (sessionId: string, provider: string, model: string) => Promise<void>;
  setThemeName: (name: string) => void;
  setLanguage: (language: string) => Promise<void>;
  setApprovalMode: (mode: 'read-only' | 'agent' | 'agent-full') => Promise<void>;
  setStatusMessage: (message: string) => void;
  setCurrentModelName: (name: string) => void;
  openModelWizard: () => void;
  onSubmit: (text: string) => void;
  exit: () => void;
  enableSkill: (name: string) => Promise<void>;
  disableSkill: (name: string) => Promise<void>;
  getSkillMarkdown: (name: string) => string;
}

export function handleSlashCommandSubmit({
  text,
  buffer,
  availableModels,
  availableThemes,
  sessionId,
  setModel,
  setThemeName,
  setLanguage,
  setApprovalMode,
  setStatusMessage,
  setCurrentModelName,
  openModelWizard,
  onSubmit,
  exit,
  enableSkill,
  disableSkill,
  getSkillMarkdown,
}: SlashCommandSubmitDeps): boolean {
  if (text.startsWith('/skill ')) {
    const parts = text.split(' ');
    if (parts[1] === 'enable' && parts[2]) {
      enableSkill(parts[2])
        .then(() => setStatusMessage(`Enabled skill ${parts[2]}`))
        .catch((e) => setStatusMessage(`Failed: ${e}`));
      setTimeout(() => setStatusMessage(''), 3000);
      buffer.setText('');
      return true;
    }
    if (parts[1] === 'disable' && parts[2]) {
      disableSkill(parts[2])
        .then(() => setStatusMessage(`Disabled skill ${parts[2]}`))
        .catch((e) => setStatusMessage(`Failed: ${e}`));
      setTimeout(() => setStatusMessage(''), 3000);
      buffer.setText('');
      return true;
    }
    if (parts[1] === 'list') {
      // Just clear buffer, the menu shows the list.
      buffer.setText('');
      return true;
    }
  }

  // One-off skill execution
  const matchSkill = text.match(/^\/([a-zA-Z0-9_-]+)(.*)$/);
  if (matchSkill) {
    const name = matchSkill[1];
    const args = matchSkill[2];
    // Ignore standard commands
    if (!['model', 'theme', 'language', 'approval', 'exit', 'session', 'mcp', 'skill'].includes(name)) {
      try {
        const markdown = getSkillMarkdown(name);
        if (markdown) {
          const prompt = `<skill_instruction name="${name}">\n${markdown}\n</skill_instruction>\n\n${args.trim()}`;
          onSubmit(prompt);
          buffer.setText('');
          return true;
        }
      } catch (e) {
        // Not a skill
      }
    }
  }

  if (text.startsWith('/model ')) {
    const modelName = text.replace('/model ', '').trim();
    if (modelName === 'add') {
      openModelWizard();
      buffer.setText('');
      return true;
    }
    const model = availableModels.find((m) => m.model === modelName);
    if (model) {
      setModel(sessionId, model.provider, model.model)
        .then(() => {
          setStatusMessage(`Switched model to ${model.model}`);
          setCurrentModelName(model.model);
        })
        .catch((e) => setStatusMessage(`Failed to switch model: ${e}`));
      setTimeout(() => setStatusMessage(''), 3000);
      buffer.setText('');
      return true;
    }
  }

  if (text.startsWith('/theme ')) {
    const themeName = text.replace('/theme ', '').trim();
    if (availableThemes.find((t) => t.name === themeName)) {
      setThemeName(themeName);
      setStatusMessage(`Switched theme to ${themeName}`);
      setTimeout(() => setStatusMessage(''), 3000);
      buffer.setText('');
      return true;
    }
  }

  if (text.startsWith('/language ')) {
    const languageName = text.replace('/language ', '').trim();
    if (languageName === 'en' || languageName === 'zh-CN') {
      setLanguage(languageName)
        .then(() => {
          // Immediately switch the UI language
          i18n.changeLanguage(languageName);
          setStatusMessage(`Switched language to ${languageName}`);
        })
        .catch((e) => setStatusMessage(`Failed to switch language: ${e}`));
      setTimeout(() => setStatusMessage(''), 3000);
      buffer.setText('');
      return true;
    }
  }

  if (text.startsWith('/approval ')) {
    const mode = text.replace('/approval ', '').trim() as 'read-only' | 'agent' | 'agent-full';
    if (mode === 'read-only' || mode === 'agent' || mode === 'agent-full') {
      setApprovalMode(mode)
        .then(() => setStatusMessage(`Switched approval mode to ${mode}`))
        .catch((e) => setStatusMessage(`Failed to switch approval mode: ${e}`));
      setTimeout(() => setStatusMessage(''), 3000);
      buffer.setText('');
      return true;
    }
  }

  if (text.trim() === '/exit') {
    buffer.setText('');
    setTimeout(() => exit(), 10);
    return true;
  }

  return false;
}

interface SlashMenuProps {
  options: SlashCommand[];
  selectedIndex: number;
}

export function SlashMenu({ options, selectedIndex }: SlashMenuProps) {
  const { t } = useTranslation();
  const { theme } = useTheme();

  return (
    <Box
      flexDirection="column"
      borderStyle="round"
      borderColor={theme.colors.border}
      paddingX={1}
      marginBottom={0}
    >
      {options.length === 0 ? (
        <Box>
          <Text color={theme.colors.dimText}>No matches</Text>
        </Box>
      ) : (
        options.map((option, index) => (
          <Box key={option.name}>
            <Text color={index === selectedIndex ? theme.colors.success : theme.colors.text}>
              {index === selectedIndex ? '> ' : '  '}
            </Text>
            <Text
              color={index === selectedIndex ? theme.colors.success : theme.colors.primary}
              bold={index === selectedIndex}
            >
              /{option.name}
            </Text>
            <Box marginLeft={2}>
              <Text color={theme.colors.dimText}>{t(`commands.${option.name}`, option.description)}</Text>
            </Box>
            {option.isActive && (
              <Text color={theme.colors.success}> (current)</Text>
            )}
          </Box>
        ))
      )}
    </Box>
  );
}
