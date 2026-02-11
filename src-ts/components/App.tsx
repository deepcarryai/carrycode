import React, { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Box, useApp, useStdout } from 'ink';
import { ThemeProvider } from '../theme/index.js';
import { useRustBridge } from '../hooks/useRustBridge.js';
import { useKeypress } from '../hooks/useKeypress.js';
import { createWelcomeMessage } from './WelcomeBanner.js';
import { InputArea } from './InputArea.js';
import { OutputArea } from './OutputArea.js';
import { ProcessArea } from './ProcessArea.js';
import { ToolConfirmMenu } from './ToolConfirmMenu.js';
import { truncateToWidthWithEllipsis } from '../utils/textUtils.js';
import type {
  CoreConfirmationRequest,
  CoreEvent,
  Message,
  StageSegment,
  ResponseStage,
  ToolCallLog,
  ToolOperation,
} from '../types/index.js';

export function App() {
  const { t, i18n } = useTranslation();
  const { askAgent, cancelAgent, createSessionId, confirmTool, getSessionHistory, getAgentMode } =
    useRustBridge();
  const [messages, setMessages] = useState<Message[]>([createWelcomeMessage()]);
  const [loading, setLoading] = useState(false);
  const [sessionId, setSessionId] = useState(() => createSessionId());
  const [agentMode, setAgentMode] = useState<'plan' | 'build'>('build');
  const [confirmationRequest, setConfirmationRequest] = useState<CoreConfirmationRequest | null>(null);
  const [liveMessage, setLiveMessage] = useState<Message | null>(null);
  const [staticEpoch, setStaticEpoch] = useState(0);
  const resizeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const { stdout } = useStdout();
  const { exit } = useApp();

  const lastEscAtRef = useRef<number>(0);
  const lastCtrlCAtRef = useRef<number>(0);

  // Track last known dimensions to prevent unnecessary redraws (e.g. on tab switch)
  const lastDimensionsRef = useRef({ columns: 0, rows: 0 });


  useEffect(() => {
    const out: any = stdout as any;
    if (!out || typeof out.on !== 'function') return;

    // Initialize dimensions
    if (out.columns && out.rows) {
      lastDimensionsRef.current = { columns: out.columns, rows: out.rows };
    }

    const handleResize = () => {
      const currentCols = out.columns;
      const currentRows = out.rows;

      // If dimensions haven't actually changed, ignore the event
      // This prevents scroll jumping when switching terminal tabs
      if (
        currentCols === lastDimensionsRef.current.columns &&
        currentRows === lastDimensionsRef.current.rows
      ) {
        return;
      }

      lastDimensionsRef.current = { columns: currentCols, rows: currentRows };

      if (resizeTimerRef.current) {
        clearTimeout(resizeTimerRef.current);
      }
      resizeTimerRef.current = setTimeout(() => {
        try {
          out.write('\x1b[2J\x1b[3J\x1b[H');
        } catch {
        }
        setStaticEpoch((e) => e + 1);
      }, 80);
    };

    out.on('resize', handleResize);
    return () => {
      if (resizeTimerRef.current) {
        clearTimeout(resizeTimerRef.current);
        resizeTimerRef.current = null;
      }
      if (typeof out.off === 'function') out.off('resize', handleResize);
      else if (typeof out.removeListener === 'function') out.removeListener('resize', handleResize);
    };
  }, [stdout]);

  useKeypress(
    (key) => {
      const now = Date.now();
      const doublePressWindowMs = 650;

      if (key.ctrl && key.name === 'c') {
        if (now - lastCtrlCAtRef.current <= doublePressWindowMs) {
          lastCtrlCAtRef.current = 0;
          setTimeout(() => exit(), 0);
          return;
        }
        lastCtrlCAtRef.current = now;
        return;
      }

      if (key.name === 'escape' && (loading || confirmationRequest)) {
        if (now - lastEscAtRef.current <= doublePressWindowMs) {
          lastEscAtRef.current = 0;
          setConfirmationRequest(null);
          setLiveMessage(null);
          setLoading(false);
          cancelAgent(sessionId).catch(() => { });
          return;
        }
        lastEscAtRef.current = now;
      }
    },
    { isActive: true },
  );

  async function handleConfirmation(decision: string) {
    if (!confirmationRequest) return;
    const requestId = confirmationRequest.requestId;
    setConfirmationRequest(null);
    try {
      await confirmTool(sessionId, { requestId, decision });
    } catch {
    }
  }

  const handleWelcome = () => {
    setMessages(prev => [
      ...prev,
      createWelcomeMessage()
    ]);
  };

  type ProviderMessage = { role: string; content: string };

  const truncateText = (text: string, maxChars: number) => {
    const trimmed = text.trim();
    return truncateToWidthWithEllipsis(trimmed, maxChars, '…');
  };

  const buildSessionSummary = (
    history: ProviderMessage[],
    activeSessionId: string,
    overrides?: { messageCount?: string; description?: string },
  ): Message[] => {
    const firstUserMessage = history.find(
      (m) => m.role === 'user' && !m.content.startsWith('ToolResult:\n') && m.content.trim().length > 0,
    )?.content;
    const description =
      overrides?.description ??
      (firstUserMessage ? truncateText(firstUserMessage, 80) : t('session.no_description'));
    const messageCount = overrides?.messageCount ?? String(history.length);
    const content =
      `${t('session.summary_title')}\n` +
      `${t('session.id')}: ${activeSessionId}\n` +
      `${t('session.message_count')}: ${messageCount}\n` +
      `${t('session.description')}: ${description}`;

    return [
      {
        question: `Session ${activeSessionId}`,
        segments: [
          {
            stage: '__ANSWERING__',
            title: t('session.summary_title'),
            content,
            tools: [],
          },
        ],
        startTime: Date.now(),
      },
    ];
  };

  const buildMessagesFromHistory = (history: ProviderMessage[]): Message[] => {
    const out: Message[] = [];
    let current: Message | null = null;
    const baseTime = Date.now();

    for (let i = 0; i < history.length; i++) {
      const m = history[i];
      if (m.role === 'user') {
        if (m.content.startsWith('ToolResult:\n') && current) {
          current.segments.push({
            stage: '__ANSWERING__',
            title: t('status.answering'),
            content: m.content,
            tools: [],
          });
          continue;
        }
        current = { question: m.content, segments: [], startTime: baseTime + i };
        out.push(current);
        continue;
      }

      if (m.role === 'assistant') {
        if (!current) {
          current = { question: '', segments: [], startTime: baseTime + i };
          out.push(current);
        }
        current.segments.push({
          stage: '__ANSWERING__',
          title: t('status.answering'),
          content: m.content,
          tools: [],
        });
      }
    }

    return out;
  };

  async function switchSession(targetSessionId: string) {
    setLoading(true);
    setConfirmationRequest(null);
    const restoreStartTime = Date.now();
    setLiveMessage({
      question: '',
      segments: [
        {
          stage: '__ANSWERING__',
          title: t('status.session_restoring'),
          content: '',
          tools: [],
        },
      ],
      startTime: restoreStartTime,
    });
    setSessionId(targetSessionId);
    try {
      try {
        const mode = await getAgentMode(targetSessionId);
        if (mode === 'plan' || mode === 'build') setAgentMode(mode);
      } catch {
      }
      const history = await getSessionHistory(targetSessionId);
      setMessages((prev) => [
        ...prev,
        ...buildSessionSummary(history, targetSessionId),
        ...buildMessagesFromHistory(history),
      ]);
    } catch {
      setMessages((prev) => [
        ...prev,
        {
          question: `Session ${targetSessionId}`,
          segments: [
            {
              stage: '__ANSWERING__',
              title: t('session.summary_title'),
              content: t('session.restore_failed'),
              tools: [],
            },
          ],
          startTime: Date.now(),
        },
      ]);
    } finally {
      setLoading(false);
      setLiveMessage(null);
    }
  }

  async function handleSubmit(input: string) {
    if (confirmationRequest) {
      return;
    }

    const trimmed = input.trim();
    if (trimmed.startsWith('/session ')) {
      const target = trimmed.replace(/^\/session\s+/, '').trim();
      if (target.toLowerCase() === 'new') {
        const newId = createSessionId();
        await switchSession(newId);
        return;
      }
      if (target.length > 0) {
        await switchSession(target);
      }
      return;
    }

    setLoading(true);
    const question = input;
    const requestStartTime = Date.now();

    setMessages(prev => [...prev, { question, segments: [], startTime: requestStartTime }]);

    setLiveMessage({ question, segments: [], startTime: requestStartTime });

    try {
      let currentStage: ResponseStage | null = null;
      let currentAnswerStageTitle = t('status.explored');
      let currentProcessStageTitle = t('status.processing');
      let stageTextBuffer = '';
      let ended = false;
      let inToolSegment = false;
      let currentToolOperation: ToolOperation | undefined = undefined;
      let toolLogsBuffer: ToolCallLog[] = [];

      const getAnswerStageTitle = (op: ToolOperation) => {
        if (op === '__EXPLORED__') return t('status.explored');
        if (op === '__EDITED__') return t('status.edited');
        if (op === '__TODO__') return t('status.todo');
        return t('status.bash');
      };

      const getProcessToolTitle = (op: ToolOperation) => {
        if (op === '__EXPLORED__') return t('status.exploring');
        if (op === '__EDITED__') return t('status.editing');
        if (op === '__TODO__') return t('status.todo');
        return t('status.bash');
      };

      const getTitle = (s: ResponseStage) => {
        if (s === '__THINKING__') return t('status.thinking');
        if (s === '__ANSWERING__') return t('status.answering');
        return t('status.processing');
      };

      const getProcessTitle = (s: ResponseStage) => {
        if (s === '__THINKING__') return t('status.thinking');
        if (s === '__ANSWERING__') return t('status.answering');
        return t('status.processing');
      };

      const appendSegment = (segment: StageSegment) => {
        setMessages((prev) => {
          const lastMsgIndex = prev.length - 1;
          if (lastMsgIndex < 0) return prev;
          const lastMsg = prev[lastMsgIndex];
          const next = [...prev];
          next[lastMsgIndex] = {
            ...lastMsg,
            segments: [...lastMsg.segments, segment],
          };
          return next;
        });
      };

      const flushCurrentStage = () => {
        if (!currentStage) return;

        const content = stageTextBuffer;
        const hasContent = content.replace(/\s+/g, '').length > 0;
        const hasTools = inToolSegment && toolLogsBuffer.length > 0;
        if (!hasContent && !hasTools) return;

        appendSegment({
          stage: currentStage,
          title: inToolSegment ? currentAnswerStageTitle : getTitle(currentStage),
          content,
          tools: inToolSegment ? toolLogsBuffer : [],
          toolOperation: inToolSegment ? currentToolOperation : undefined,
        });
        toolLogsBuffer = [];
      };

      const setProcessStage = (stage: ResponseStage) => {
        const newStartTime = Date.now();
        setLiveMessage({
          question,
          segments: [
            {
              stage,
              title: getProcessTitle(stage),
              content: '',
              tools: [],
            },
          ],
          startTime: newStartTime,
        });
      };

      type StageEndMarker = '__THINKING_END__' | '__ANSWERING_END__';
      type ToolOperationEnd = '__EXPLORED_END__' | '__EDITED_END__' | '__TODO_END__' | '__BASH_END__';

      const stageEndToStage = (m: StageEndMarker): ResponseStage => {
        if (m === '__THINKING_END__') return '__THINKING__';
        return '__ANSWERING__';
      };

      const toolEndToOp = (m: ToolOperationEnd): ToolOperation => {
        if (m === '__EXPLORED_END__') return '__EXPLORED__';
        if (m === '__EDITED_END__') return '__EDITED__';
        if (m === '__TODO_END__') return '__TODO__';
        return '__BASH__';
      };

      const handleStreamEnd = () => {
        flushCurrentStage();
        currentStage = null;
        stageTextBuffer = '';
        ended = true;
        setLiveMessage(null);
      };

      const handleStageStart = (stage: ResponseStage) => {
        flushCurrentStage();
        currentStage = stage;
        inToolSegment = false;
        currentToolOperation = undefined;
        toolLogsBuffer = [];
        stageTextBuffer = '';
        setProcessStage(stage);
      };

      const handleStageEnd = (marker: StageEndMarker) => {
        const stage = stageEndToStage(marker);
        if (currentStage !== stage) return;
        if (inToolSegment) return;
        flushCurrentStage();
        currentStage = null;
        stageTextBuffer = '';
        setLiveMessage(null);
      };

      const handleToolOperationTag = (op: ToolOperation) => {
        flushCurrentStage();
        currentAnswerStageTitle = getAnswerStageTitle(op);
        currentProcessStageTitle = getProcessToolTitle(op);
        currentStage = '__ANSWERING__';
        inToolSegment = true;
        currentToolOperation = op;
        toolLogsBuffer = [];
        stageTextBuffer = '';
        const newStartTime = Date.now();
        setLiveMessage({
          question,
          segments: [
            {
              stage: '__ANSWERING__',
              title: currentProcessStageTitle,
              content: '',
              tools: [],
              toolOperation: op,
            },
          ],
          startTime: newStartTime,
        });
      };

      const handleToolOperationEndTag = (marker: ToolOperationEnd) => {
        if (!inToolSegment) return;
        const op = toolEndToOp(marker);
        if (currentToolOperation && currentToolOperation !== op) return;
        flushCurrentStage();
        currentStage = null;
        inToolSegment = false;
        currentToolOperation = undefined;
        toolLogsBuffer = [];
        stageTextBuffer = '';
        setLiveMessage(null);
      };

      const handleText = (text: string) => {
        if (!text) return;
        if (!currentStage) return;
        if (inToolSegment) return;
        stageTextBuffer += text;
      };

      const stageToEndMarker = (stage: ResponseStage): StageEndMarker => {
        if (stage === '__THINKING__') return '__THINKING_END__';
        return '__ANSWERING_END__';
      };

      const opToEndMarker = (op: ToolOperation): ToolOperationEnd => {
        if (op === '__EXPLORED__') return '__EXPLORED_END__';
        if (op === '__EDITED__') return '__EDITED_END__';
        if (op === '__TODO__') return '__TODO_END__';
        return '__BASH_END__';
      };

      const appendToolOutput = (event: CoreEvent) => {
        const op = (event.toolOperation ?? currentToolOperation ?? '__EXPLORED__') as ToolOperation;
        const toolName = String(event.toolName ?? '');
        const paramsSummary = String(event.argsSummary ?? '');
        const responseSummary =
          op === '__TODO__'
            ? String(event.responseSummary ?? event.displayText ?? '')
            : String(event.displayText ?? event.responseSummary ?? '');
        const status =
          typeof event.success === 'boolean' ? (event.success ? 'ok' : 'error') : undefined;
        toolLogsBuffer = [
          ...toolLogsBuffer,
          {
            operation: op,
            toolName,
            paramsSummary,
            responseSummary,
            status,
          },
        ];
      };

      await askAgent(
        sessionId,
        question,
        function onEvent(event: CoreEvent) {
          if (!event || typeof event !== 'object') {
            return;
          }
          const eventType = event.eventType;
          const toolOperation = event.toolOperation;
          if (eventType === 'Text') {
            handleText(String(event.text ?? ''));
            return;
          }
          if (eventType === 'StageStart' && event.stage) {
            handleStageStart(event.stage);
            return;
          }
          if (eventType === 'StageEnd' && event.stage) {
            handleStageEnd(stageToEndMarker(event.stage));
            return;
          }
          if (eventType === 'ToolStart' && toolOperation) {
            handleToolOperationTag(toolOperation);
            return;
          }
          if (eventType === 'ToolOutput') {
            if (toolOperation && !inToolSegment) {
              handleToolOperationTag(toolOperation);
            }
            appendToolOutput(event);
            return;
          }
          if (eventType === 'ToolEnd' && toolOperation) {
            handleToolOperationEndTag(opToEndMarker(toolOperation));
            return;
          }
          if (eventType === 'End') {
            handleStreamEnd();
            return;
          }
          if (eventType === 'Error') {
            const msg = String(event.errorMessage ?? 'Unknown error');
            appendSegment({
              stage: '__ANSWERING__',
              title: t('status.error'),
              content: `Error: ${msg}`,
              tools: [],
            });
          }
        },
        (req) => setConfirmationRequest(req),
      );

      if (!ended) {
        flushCurrentStage();
        setLiveMessage(null);
      }

      const endTime = Date.now();
      const duration = endTime - requestStartTime;

      setMessages(prev => {
        const lastIndex = prev.length - 1;
        if (lastIndex < 0) return prev;
        const next = [...prev];
        next[lastIndex] = { ...next[lastIndex], duration };
        return next;
      });

    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : 'Unknown error';
      setMessages(prev => {
        const lastIndex = prev.length - 1;
        if (lastIndex < 0) return prev;
        const lastMsg = prev[lastIndex];
        const errorSegment: StageSegment = {
          stage: '__ANSWERING__',
          title: t('status.error'),
          content: `Error: ${errorMsg}`,
          tools: []
        };
        const next = [...prev];
        next[lastIndex] = {
          ...lastMsg,
          segments: [...lastMsg.segments, errorSegment]
        };
        return next;
      });
    } finally {
      setLoading(false);
    }
  }

  return (
    <ThemeProvider>
      <Box flexDirection="column" height="100%">
        <Box flexGrow={1} flexDirection="column">
          <OutputArea messages={messages} staticEpoch={staticEpoch} />
        </Box>

        <ProcessArea
          sessionId={sessionId}
          agentMode={agentMode}
          segment={liveMessage?.segments[liveMessage.segments.length - 1]}
          startTime={liveMessage?.startTime}
          isIdle={!loading}
        />
        {confirmationRequest ? (
          <ToolConfirmMenu request={confirmationRequest} onConfirm={handleConfirmation} />
        ) : (
          <InputArea onSubmit={handleSubmit} disabled={loading} sessionId={sessionId} agentMode={agentMode} onAgentModeChange={setAgentMode} />
        )}
      </Box>
    </ThemeProvider>
  );
}
