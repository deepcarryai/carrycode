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
import { WelcomeConfigWizard } from './WelcomeConfigWizard.js';
import { WelcomeModelWizard } from './WelcomeModelWizard.js';
import type { AvailableModel } from '../types/index.js';
import type { SavedSessionInfo } from '../hooks/useRustBridge.js';

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
  const {
    getAvailableModels,
    setModel,
    getSavedSessions,
    getSessions,
    checkLatency,
    getApprovalMode,
    setApprovalMode,
    getAgentMode,
    setAgentMode,
    getAppConfig,
    getConfigBootstrapState,
    setLanguage,
    listAvailableSkills,
    enableSkillForSession,
    disableSkillForSession,
    getSkillMarkdown,
  } = useRustBridge();
  const { stdout } = useStdout();
  const { exit } = useApp();
  const columns = stdout?.columns ?? 80;
  const rows = 4;
  const viewportWidth = Math.max(1, columns - 6);
  const placeholder = t('input.placeholder');

  const [availableModels, setAvailableModels] = useState<AvailableModel[]>([]);
  const [availableSkills, setAvailableSkills] = useState<any[]>([]);
  const [sessions, setSessions] = useState<SavedSessionInfo[]>([]);
  const [statusMessage, setStatusMessage] = useState<string>('');
  const [currentModelName, setCurrentModelName] = useState<string | undefined>(undefined);
  const [currentApprovalMode, setCurrentApprovalMode] = useState<'read-only' | 'agent' | 'agent-full'>('agent');
  const [currentLanguage, setCurrentLanguage] = useState<string>(() => {
    try {
      return getConfigBootstrapState().runtimeLanguage || 'en';
    } catch {
      return 'en';
    }
  });
  const [appConfig, setAppConfig] = useState(() => getAppConfig());
  const [showWelcomeWizard, setShowWelcomeWizard] = useState(() => {
    try {
      return Boolean(getConfigBootstrapState().needsWelcomeWizard);
    } catch {
      return false;
    }
  });
  const [showWelcomeModelWizard, setShowWelcomeModelWizard] = useState(false);
  const [history, setHistory] = useState<string[]>([]);
  const [historyIndex, setHistoryIndex] = useState<number | null>(null);
  const [draft, setDraft] = useState<string>('');

  useEffect(() => {
    if (showWelcomeWizard || showWelcomeModelWizard) {
      return;
    }
    try {
      setAvailableSkills(listAvailableSkills(sessionId));
    } catch {}
    getAvailableModels(sessionId).then(setAvailableModels).catch(() => { });
    let cancelled = false;
    (async () => {
      try {
        const saved = await getSavedSessions();
        if (!cancelled) setSessions(saved);
      } catch {
        setStatusMessage('Saved sessions unavailable (rebuild rust to enable)');
        setTimeout(() => setStatusMessage(''), 3000);
      }
    })();
    try {
      setAppConfig(getAppConfig());
    } catch {
    }
    getApprovalMode(sessionId).then(setCurrentApprovalMode).catch(() => {});
    getAgentMode(sessionId).then((mode) => onAgentModeChange?.(mode)).catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [sessionId, showWelcomeWizard, showWelcomeModelWizard]);

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
    availableSkills,
    theme.name,
    currentModelName,
    currentApprovalMode,
    currentLanguage
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
    setLanguage: async (language) => {
      await setLanguage(language);
      setCurrentLanguage(language);
    },
    setApprovalMode: async (mode) => {
      await setApprovalMode(sessionId, mode);
      setCurrentApprovalMode(mode);
    },
    setStatusMessage,
    setCurrentModelName,
    openModelWizard: () => setShowWelcomeModelWizard(true),
    onSubmit,
    exit,
  });

  useKeypress(
    (key) => {
      if (disabled) return;
      if (showWelcomeWizard || showWelcomeModelWizard) return;

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

      if (key.name === 'return' && !key.shift && !key.ctrl && !key.meta) {
        // Check for backslash at end of line (continuation)
        if (buffer.cursorCol > 0) {
          const currentLine = buffer.lines[buffer.cursorRow] ?? '';
          const cps = toCodePoints(currentLine);
          if (cps[buffer.cursorCol - 1] === '\\') {
            buffer.handleKey({ name: 'backspace', shift: false, meta: false, ctrl: false, sequence: '' });
            buffer.handleKey({ name: 'return', shift: true, meta: false, ctrl: false, sequence: '\n' });
            return;
          }
        }

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
            setLanguage: async (language) => {
              await setLanguage(language);
              setCurrentLanguage(language);
            },
            setApprovalMode: async (mode) => {
              await setApprovalMode(sessionId, mode);
              setCurrentApprovalMode(mode);
            },
            setStatusMessage,
            setCurrentModelName,
            openModelWizard: () => setShowWelcomeModelWizard(true),
            onSubmit,
            exit,
            enableSkill: (name) => enableSkillForSession(sessionId, name),
            disableSkill: (name) => disableSkillForSession(sessionId, name),
            getSkillMarkdown: (name) => getSkillMarkdown(sessionId, name),
          })
        ) {
          return;
        }

        if (text.trim().length > 0) {
          setHistory((prev) => {
            if (prev.length > 0 && prev[prev.length - 1] === text) {
              return prev;
            }
            return [...prev, text];
          });
        }
        setHistoryIndex(null);
        setDraft('');

        // 提交时自动展开所有折叠块
        const expandedText = buffer.expandAllBlocks();
        onSubmit(expandedText);
        buffer.setText('');
        return;
      }
      
      if (!menuState.isOpen) {
        if (key.name === 'up' && buffer.cursorRow === 0) {
          if (historyIndex === null) {
            if (history.length > 0) {
              setDraft(buffer.text);
              const newIndex = history.length - 1;
              setHistoryIndex(newIndex);
              buffer.setText(history[newIndex]);
            }
          } else if (historyIndex > 0) {
            const newIndex = historyIndex - 1;
            setHistoryIndex(newIndex);
            buffer.setText(history[newIndex]);
          }
          return;
        }

        if (key.name === 'down' && buffer.cursorRow === buffer.lineCount - 1) {
          if (historyIndex !== null) {
            if (historyIndex < history.length - 1) {
              const newIndex = historyIndex + 1;
              setHistoryIndex(newIndex);
              buffer.setText(history[newIndex]);
            } else {
              setHistoryIndex(null);
              buffer.setText(draft);
            }
            return;
          }
        }
      }

      buffer.handleKey(key);
    },
    { isActive: !disabled && !showWelcomeWizard && !showWelcomeModelWizard },
  );

  const lines = buffer.viewportVisualLines;
  const cursor = buffer.visualCursor;

  // 辅助函数：渲染带有折叠块高亮的行
  const renderLineWithCollapsedBlocks = (line: string, lineIndex: number, isCursorRow: boolean) => {
    // 查找该行是否包含折叠块占位符
    const logicalLineIndex = buffer.cursorRow - cursor.row + lineIndex;
    const collapsedBlock = buffer.collapsedBlocks.find(
      (b) => b.startLine === logicalLineIndex && !b.isExpanded && line.includes(b.placeholder),
    );

    if (!collapsedBlock) {
      // 没有折叠块，正常渲染
      if (!isCursorRow) {
        return line;
      }
      const { before, cursorChar, after } = splitByColumn(line, cursor.col);
      return (
        <>
          {before}
          <Text inverse>{cursorChar.length === 0 ? ' ' : cursorChar}</Text>
          {after}
        </>
      );
    }

    // 有折叠块，高亮显示占位符
    const placeholderIndex = line.indexOf(collapsedBlock.placeholder);
    const before = line.slice(0, placeholderIndex);
    const after = line.slice(placeholderIndex + collapsedBlock.placeholder.length);

    if (!isCursorRow) {
      return (
        <>
          {before}
          <Text color="cyan" dimColor>
            {collapsedBlock.placeholder}
          </Text>
          {after}
        </>
      );
    }

    // 光标在该行，需要处理光标位置
    const { before: cursorBefore, cursorChar, after: cursorAfter } = splitByColumn(line, cursor.col);
    const cursorInPlaceholder =
      cursor.col >= placeholderIndex && cursor.col < placeholderIndex + collapsedBlock.placeholder.length;

    if (cursorInPlaceholder) {
      // 光标在占位符内
      const placeholderBefore = line.slice(placeholderIndex, placeholderIndex + (cursor.col - placeholderIndex));
      const placeholderAfter = line.slice(placeholderIndex + (cursor.col - placeholderIndex) + cursorChar.length, placeholderIndex + collapsedBlock.placeholder.length);

      return (
        <>
          {before}
          <Text color="cyan" dimColor>
            {placeholderBefore}
          </Text>
          <Text inverse>{cursorChar.length === 0 ? ' ' : cursorChar}</Text>
          <Text color="cyan" dimColor>
            {placeholderAfter}
          </Text>
          {after}
        </>
      );
    }

    // 光标不在占位符内
    if (cursor.col < placeholderIndex) {
      // 光标在占位符之前
      return (
        <>
          {cursorBefore}
          <Text inverse>{cursorChar.length === 0 ? ' ' : cursorChar}</Text>
          {line.slice(cursor.col + cursorChar.length, placeholderIndex)}
          <Text color="cyan" dimColor>
            {collapsedBlock.placeholder}
          </Text>
          {after}
        </>
      );
    } else {
      // 光标在占位符之后
      return (
        <>
          {before}
          <Text color="cyan" dimColor>
            {collapsedBlock.placeholder}
          </Text>
          {line.slice(placeholderIndex + collapsedBlock.placeholder.length, cursor.col)}
          <Text inverse>{cursorChar.length === 0 ? ' ' : cursorChar}</Text>
          {cursorAfter}
        </>
      );
    }
  };

  return (
    <Box flexDirection="column" width="100%">
      <Box flexDirection="column" borderStyle="round" borderColor={theme.colors.border} paddingX={2}>
        {lines.map((line, i) => {
          const showPlaceholder = buffer.isEmpty && i === 0 && placeholder.length > 0;
          const renderedLine = showPlaceholder ? placeholder : line;
          const isCursorRow = i === cursor.row;

          return (
            <Text key={i} dimColor={showPlaceholder} color={theme.colors.text}>
              {showPlaceholder ? renderedLine : renderLineWithCollapsedBlocks(renderedLine, i, isCursorRow)}
            </Text>
          );
        })}
      </Box>
      {statusMessage && <Text color={theme.colors.success}>{statusMessage}</Text>}
      {showWelcomeWizard ? (
        <WelcomeConfigWizard
          sessionId={sessionId}
          onFinished={() => {
            setShowWelcomeWizard(false);
            setStatusMessage('Welcome config saved');
            setTimeout(() => setStatusMessage(''), 2500);
          }}
          onCancelled={() => {
            setShowWelcomeWizard(false);
          }}
        />
      ) : showWelcomeModelWizard ? (
        <WelcomeModelWizard
          sessionId={sessionId}
          onFinished={() => {
            setShowWelcomeModelWizard(false);
            setStatusMessage('Model config saved');
            setTimeout(() => setStatusMessage(''), 2500);
            getAvailableModels(sessionId).then(setAvailableModels).catch(() => { });
          }}
          onCancelled={() => {
            setShowWelcomeModelWizard(false);
          }}
        />
      ) : (
        menuState.isOpen && <SlashMenu options={menuState.options} selectedIndex={menuSelectedIndex} />
      )}
    </Box>
  );
}
