import React from 'react';
import { useTranslation } from 'react-i18next';
import { Box, Text } from 'ink';
import { useTheme } from '../theme/index.js';
import type { Key } from '../hooks/useKeypress.js';
import type { TextBuffer } from '../hooks/useTextBuffer.js';
import type { AvailableModel } from '../types/index.js';

export interface SlashCommand {
  name: string;
  description: string;
  subCommands?: SlashCommand[];
  isActive?: boolean;
}

export const SLASH_COMMANDS: SlashCommand[] = [
  {
    name: 'approvals',
    description: 'Select Approval Mode',
    subCommands: [
      { name: 'read-only', description: 'Requires approval to edit files and run commands.' },
      { name: 'agent', description: 'Read and edit files, and run commands.' },
      { name: 'agent-full', description: 'Edit outside workspace and run network commands. Use with caution.' },
    ],
  },
  {
    name: 'config',
    description: 'configure system settings',
    subCommands: [
      {
        name: 'model',
        description: 'select LLM model',
        subCommands: [], // Populated dynamically
      },
      {
        name: 'theme',
        description: 'select UI theme',
        subCommands: [], // Populated dynamically
      },
    ],
  },
  {
    name: 'exit',
    description: 'exit the application',
  },
  {
    name: 'resume',
    description: 'resume a saved chat',
  },
  {
    name: 'sessions',
    description: 'manage sessions',
    subCommands: [
      { name: 'New', description: 'Start a new session' },
      // Other sessions populated dynamically
    ],
  },
  {
    name: 'skills',
    description: 'use skills to improve how Codex performs specific tasks',
  },
  {
    name: 'mcp',
    description: 'manage mcp servers',
    subCommands: [], // Populated dynamically
  },
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
  sessions: string[],
  mcpServers: Record<string, any>,
  currentThemeName?: string,
  currentModelName?: string,
  currentApprovalMode?: string
): SlashMenuState {
  return React.useMemo(() => {
    if (!text.startsWith('/')) {
      return { isOpen: false, options: [], level: 0, prefix: '' };
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

    // Check level 3 (e.g. /config model gpt-4)
    const matchL3 = text.match(/^\/([a-zA-Z0-9_-]+)\s+([a-zA-Z0-9_-]+)\s+(.*)$/);
    if (matchL3) {
      // We generally don't have level 3 autocomplete yet, unless specific subcommands have subcommands.
      // For now, config model/theme are leaf nodes in terms of selection, but we can filter them.
      const cmdName = matchL3[1];
      const subCmdName = matchL3[2];
      const query = matchL3[3] || '';

      if (cmdName === 'config') {
        if (subCmdName === 'model') {
          const options = availableModels.map(m => ({
            name: m.model,
            description: `${m.provider} ${m.model}`,
            isActive: currentModelName ? m.model.toLowerCase() === currentModelName.toLowerCase() : false
          })).filter(o => o.name.toLowerCase().startsWith(query.toLowerCase()));
          return { isOpen: options.length > 0, options, level: 3, prefix: `/config model ` };
        }
        if (subCmdName === 'theme') {
          const options = availableThemes.map(t => ({
            name: t.name,
            description: t.name,
            isActive: currentThemeName ? t.name === currentThemeName : false
          })).filter(o => o.name.toLowerCase().startsWith(query.toLowerCase()));
          return { isOpen: options.length > 0, options, level: 3, prefix: `/config theme ` };
        }
      }
      return { isOpen: false, options: [], level: 0, prefix: '' };
    }

    // Check level 2 (e.g. /config model)
    const matchL2 = text.match(/^\/([a-zA-Z0-9_-]+)\s+(.*)$/);
    if (matchL2) {
      const cmdName = matchL2[1];
      const subQuery = matchL2[2] || '';

      const cmd = SLASH_COMMANDS.find((c) => c.name === cmdName);
      if (cmd) {
        if (cmdName === 'sessions') {
          // Dynamic session list
          const dynamicSessions = [
            { name: 'New', description: 'session_new' },
            ...sessions.map(s => ({ name: s, description: 'session_switch' }))
          ];
          const filtered = dynamicSessions.filter(s => s.name.toLowerCase().startsWith(subQuery.toLowerCase()));
          return { isOpen: true, options: filtered, level: 2, prefix: `/sessions ` };
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
            cmdName === 'approvals'
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

    // Check level 1 (e.g. /config)
    const query = text.slice(1);
    if (query.includes(' ')) return { isOpen: false, options: [], level: 0, prefix: '' };

    const filtered = SLASH_COMMANDS.filter((c) => c.name.toLowerCase().startsWith(query.toLowerCase()));
    return {
      isOpen: filtered.length > 0,
      options: filtered,
      level: 1,
      prefix: '/',
    };
  }, [text, availableModels, availableThemes, sessions, mcpServers, currentThemeName, currentModelName, currentApprovalMode]);
}

export interface SlashMenuAction {
  type: 'execute' | 'autocomplete';
  payload: string;
  isConfigModel?: boolean;
  isConfigTheme?: boolean;
  isSession?: boolean;
  isApprovals?: boolean;
  isExit?: boolean;
}

export function getSlashMenuAction(
  menuState: SlashMenuState,
  selected: SlashCommand
): SlashMenuAction {
  const newValue = `${menuState.prefix}${selected.name}`;

  const isConfigModel = menuState.prefix === '/config model ';
  const isConfigTheme = menuState.prefix === '/config theme ';
  const isSession = menuState.prefix === '/sessions ';
  const isApprovals = menuState.prefix === '/approvals ';
  const isExit = menuState.prefix === '/' && selected.name === 'exit';

  if (isConfigModel || isConfigTheme || isSession || isApprovals || isExit) {
    return {
      type: 'execute',
      payload: newValue,
      isConfigModel,
      isConfigTheme,
      isSession,
      isApprovals,
      isExit
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
  setApprovalMode: (mode: 'read-only' | 'agent' | 'agent-full') => Promise<void>;
  setStatusMessage: (message: string) => void;
  setCurrentModelName: (name: string) => void;
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
  setApprovalMode,
  setStatusMessage,
  setCurrentModelName,
  onSubmit,
  exit,
}: SlashMenuHandlersDeps): {
  handleMenuSelect: (selected: SlashCommand) => void;
  handleMenuEscape: () => void;
} {
  const handleMenuSelect = (selected: SlashCommand) => {
    const action = getSlashMenuAction(menuState, selected);

    if (action.type === 'execute') {
      if (action.isConfigModel) {
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
      } else if (action.isConfigTheme) {
        setThemeName(selected.name);
        setStatusMessage(`Switched theme to ${selected.name}`);
        setTimeout(() => setStatusMessage(''), 3000);
      } else if (action.isApprovals) {
        const mode = selected.name as 'read-only' | 'agent' | 'agent-full';
        setApprovalMode(mode)
          .then(() => {
            setStatusMessage(`Switched approval mode to ${mode}`);
          })
          .catch((e) => setStatusMessage(`Failed to switch approval mode: ${e}`));
        setTimeout(() => setStatusMessage(''), 3000);
      } else if (action.isSession) {
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
    const matchL3 = text.match(/^\/([a-zA-Z0-9_-]+)\s+([a-zA-Z0-9_-]+)\s+/);
    if (matchL3) {
      const cmd = matchL3[1];
      buffer.setText(`/${cmd} `);
      return;
    }

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
  setApprovalMode: (mode: 'read-only' | 'agent' | 'agent-full') => Promise<void>;
  setStatusMessage: (message: string) => void;
  setCurrentModelName: (name: string) => void;
  onSubmit: (text: string) => void;
  exit: () => void;
}

export function handleSlashCommandSubmit({
  text,
  buffer,
  availableModels,
  availableThemes,
  sessionId,
  setModel,
  setThemeName,
  setApprovalMode,
  setStatusMessage,
  setCurrentModelName,
  onSubmit,
  exit,
}: SlashCommandSubmitDeps): boolean {
  if (text.startsWith('/config model ')) {
    const modelName = text.replace('/config model ', '').trim();
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

  if (text.startsWith('/config theme ')) {
    const themeName = text.replace('/config theme ', '').trim();
    if (availableThemes.find((t) => t.name === themeName)) {
      setThemeName(themeName);
      setStatusMessage(`Switched theme to ${themeName}`);
      setTimeout(() => setStatusMessage(''), 3000);
      buffer.setText('');
      return true;
    }
  }

  if (text.startsWith('/approvals ')) {
    const mode = text.replace('/approvals ', '').trim() as 'read-only' | 'agent' | 'agent-full';
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
