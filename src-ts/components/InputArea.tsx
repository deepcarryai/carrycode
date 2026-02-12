import React, { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Box, Text, useStdout, useApp } from 'ink';
import { useKeypress } from '../hooks/useKeypress.js';
import { useTextBuffer } from '../hooks/useTextBuffer.js';
import { useTheme } from '../theme/index.js';
import { useRustBridge } from '../hooks/useRustBridge.js';
import { getCachedStringWidth, toCodePoints } from '../utils/textUtils.js';
import {
  SlashMenu,
  useSlashMenuState,
  createSlashMenuHandlers,
  handleSlashCommandSubmit,
  isSlashMenuNavigationKey,
} from './SlashMenu.js';
import type { AvailableModel } from '../types/index.js';

function splitByColumn(
  str: string,
  col: number,
): { before: string; cursorChar: string; after: string } {
  if (col <= 0) {
    const cps = toCodePoints(str);
    const cursorChar = cps[0] ?? ' ';
    const after = cps.slice(1).join('');
    return { before: '', cursorChar, after };
  }

  const cps = toCodePoints(str);
  let width = 0;
  for (let i = 0; i < cps.length; i++) {
    const w = getCachedStringWidth(cps[i]);
    if (width === col) {
      const before = cps.slice(0, i).join('');
      const cursorChar = cps[i] ?? ' ';
      const after = cps.slice(i + 1).join('');
      return { before, cursorChar, after };
    }
    if (width + w > col) {
      const before = cps.slice(0, i).join('');
      const cursorChar = cps[i] ?? ' ';
      const after = cps.slice(i + 1).join('');
      return { before, cursorChar, after };
    }
    width += w;
  }

  return { before: str, cursorChar: ' ', after: '' };
}

interface InputAreaProps {
  onSubmit: (text: string) => void;
  disabled?: boolean;
  sessionId: string;
  agentMode?: 'plan' | 'build';
  onAgentModeChange?: (mode: 'plan' | 'build') => void;
}

export function InputArea({ onSubmit, disabled = false, sessionId, agentMode, onAgentModeChange }: InputAreaProps) {
  const { t } = useTranslation();
  const { theme, availableThemes, setThemeName } = useTheme();
  const { getAvailableModels, setModel, getSavedSessions, getSessions, checkLatency, getApprovalMode, setApprovalMode, getAgentMode, setAgentMode, getAppConfig } = useRustBridge();
  const { stdout } = useStdout();
  const { exit } = useApp();
  const columns = stdout?.columns ?? 80;
  const rows = 4;
  const viewportWidth = Math.max(1, columns - 6);
  const placeholder = t('input.placeholder');

  const [availableModels, setAvailableModels] = useState<AvailableModel[]>([]);
  const [sessions, setSessions] = useState<string[]>([]);
  const [statusMessage, setStatusMessage] = useState<string>('');
  const [currentModelName, setCurrentModelName] = useState<string | undefined>(undefined);
  const [currentApprovalMode, setCurrentApprovalMode] = useState<'read-only' | 'agent' | 'agent-full'>('agent');
  const [appConfig, setAppConfig] = useState(() => getAppConfig());

  useEffect(() => {
    getAvailableModels(sessionId).then(setAvailableModels).catch(() => { });
    let cancelled = false;
    (async () => {
      let savedIds: string[] = [];
      let runtimeIds: string[] = [];

      try {
        const saved = await getSavedSessions();
        savedIds = saved.map((s) => s.sessionId);
      } catch {
        setStatusMessage('Saved sessions unavailable (rebuild rust to enable)');
        setTimeout(() => setStatusMessage(''), 3000);
      }

      try {
        runtimeIds = await getSessions();
      } catch {
      }

      const merged = Array.from(new Set([...savedIds, ...runtimeIds])).filter((s) => s.length > 0);
      if (!cancelled) setSessions(merged);
    })();
    try {
      setAppConfig(getAppConfig());
    } catch {
    }
    try {
      setCurrentApprovalMode(getApprovalMode(sessionId));
    } catch {
    }
    try {
      const mode = getAgentMode(sessionId);
      onAgentModeChange?.(mode);
    } catch {
    }
    return () => {
      cancelled = true;
    };
  }, [sessionId]);

  useEffect(() => {
    checkLatency(sessionId)
      .then((info) => setCurrentModelName(info.modelName))
      .catch(() => { });
  }, [sessionId]);

  const buffer = useTextBuffer({
    viewportWidth,
    viewportHeight: rows,
  });

  const [menuSelectedIndex, setMenuSelectedIndex] = useState(0);

  const menuState = useSlashMenuState(
    buffer.text,
    availableModels,
    availableThemes,
    sessions,
    (appConfig as any).mcp_servers || (appConfig as any).mcpServers || {},
    theme.name,
    currentModelName,
    currentApprovalMode
  );

  useEffect(() => {
    setMenuSelectedIndex(0);
  }, [menuState.options]);

  const { handleMenuSelect, handleMenuEscape } = createSlashMenuHandlers({
    menuState,
    buffer,
    availableModels,
    availableThemes,
    sessionId,
    setModel,
    setThemeName,
    setApprovalMode: async (mode) => {
      await setApprovalMode(sessionId, mode);
      setCurrentApprovalMode(mode);
    },
    setStatusMessage,
    setCurrentModelName,
    onSubmit,
    exit,
  });

  useKeypress(
    (key) => {
      if (disabled) return;

      if (key.name === 'tab' && key.shift) {
        const current = agentMode ?? 'build';
        const next: 'plan' | 'build' = current === 'plan' ? 'build' : 'plan';
        setAgentMode(sessionId, next)
          .then(() => {
            onAgentModeChange?.(next);
            setStatusMessage(`Switched agent mode to ${next}`);
          })
          .catch((e) => setStatusMessage(`Failed to switch agent mode: ${e}`));
        setTimeout(() => setStatusMessage(''), 3000);
        return;
      }

      if (menuState.isOpen && isSlashMenuNavigationKey(key)) {
        if (key.name === 'escape') {
          handleMenuEscape();
          return;
        }

        if (menuState.options.length === 0) {
          return;
        }

        if (key.name === 'up') {
          setMenuSelectedIndex((prev) => Math.max(0, prev - 1));
          return;
        }

        if (key.name === 'down') {
          setMenuSelectedIndex((prev) => Math.min(menuState.options.length - 1, prev + 1));
          return;
        }

        if ((key.name === 'return' || key.name === 'tab') && !key.shift) {
          const selected = menuState.options[menuSelectedIndex];
          if (selected) {
            handleMenuSelect(selected);
          }
          return;
        }

        return;
      }

      if (key.name === 'return' && !key.shift) {
        const text = buffer.text;

        if (
          handleSlashCommandSubmit({
            text,
            buffer,
            availableModels,
            availableThemes,
            sessionId,
            setModel,
            setThemeName,
            setApprovalMode: async (mode) => {
              await setApprovalMode(sessionId, mode);
              setCurrentApprovalMode(mode);
            },
            setStatusMessage,
            setCurrentModelName,
            onSubmit,
            exit,
          })
        ) {
          return;
        }

        onSubmit(text);
        buffer.setText('');
        return;
      }
      buffer.handleKey(key);
    },
    { isActive: !disabled },
  );

  const lines = buffer.viewportVisualLines;
  const cursor = buffer.visualCursor;

  return (
    <Box flexDirection="column" width="100%">
      <Box flexDirection="column" borderStyle="round" borderColor={theme.colors.border} paddingX={2}>
        {lines.map((line, i) => {
          const showPlaceholder = buffer.isEmpty && i === 0 && placeholder.length > 0;
          const renderedLine = showPlaceholder ? placeholder : line;

          if (i !== cursor.row) {
            return (
              <Text key={i} dimColor={showPlaceholder} color={theme.colors.text}>
                {renderedLine}
              </Text>
            );
          }

          const { before, cursorChar, after } = splitByColumn(renderedLine, cursor.col);
          return (
            <Text key={i} dimColor={showPlaceholder} color={theme.colors.text}>
              {before}
              <Text inverse>{cursorChar.length === 0 ? ' ' : cursorChar}</Text>
              {after}
            </Text>
          );
        })}
      </Box>
      {statusMessage && <Text color={theme.colors.success}>{statusMessage}</Text>}
      {menuState.isOpen && (
        <SlashMenu
          options={menuState.options}
          selectedIndex={menuSelectedIndex}
        />
      )}
    </Box>
  );
}
