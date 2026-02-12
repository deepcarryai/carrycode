import React, { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Box, Text } from 'ink';
import { StageSegment, LatencyInfo } from '../types/index.js';
import { useRustBridge } from '../hooks/useRustBridge.js';
import { useTheme } from '../theme/index.js';

interface ProcessAreaProps {
  sessionId?: string;
  segment?: StageSegment;
  startTime?: number;
  isIdle?: boolean;
  agentMode?: 'plan' | 'build';
}

const SPINNER_FRAMES = ['✦', '✧', '✩', '✪', '✫', '✬', '✭', '✮', '✯', '✰'];

export const ProcessArea: React.FC<ProcessAreaProps> = ({
  sessionId,
  segment,
  startTime,
  isIdle = false,
  agentMode,
}) => {
  const { t } = useTranslation();
  const { theme } = useTheme();
  const [frameIndex, setFrameIndex] = useState(0);
  const [elapsed, setElapsed] = useState(0);
  const [latencyInfo, setLatencyInfo] = useState<LatencyInfo | null>(null);
  const { checkLatency } = useRustBridge();

  useEffect(() => {
    if (isIdle || !startTime) {
      setElapsed(0);
      setFrameIndex(0);
      return;
    }

    setElapsed(Math.floor((Date.now() - startTime) / 1000));
    const timer = setInterval(() => {
      setFrameIndex((prev) => (prev + 1) % SPINNER_FRAMES.length);
      setElapsed(Math.floor((Date.now() - startTime) / 1000));
    }, 300);
    return () => clearInterval(timer);
  }, [startTime, isIdle]);

  useEffect(() => {
    if (!sessionId) return;

    const check = async () => {
      try {
        const info = await checkLatency(sessionId);
        setLatencyInfo(info);
      } catch (e) {
        // ignore errors
      }
    };

    check();
    const timer = setInterval(check, 5000);
    return () => clearInterval(timer);
  }, [sessionId]);

  const formatTime = (seconds: number) => {
    const m = Math.floor(seconds / 60);
    const s = seconds % 60;
    return `[${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}]`;
  };

  const getTitle = () => {
    if (isIdle) return t('status.idle');
    if (!segment) return t('status.submitting');
    return segment.title;
  };

  const getLatencyColor = (ms: number) => {
    if (ms < 200) return "green";
    if (ms < 500) return "yellow";
    return "red";
  };

  const getAgentModeLabel = () => {
    if (agentMode === 'plan') return t('agent_mode.plan');
    if (agentMode === 'build') return t('agent_mode.build');
    return '';
  };

  return (
    <Box paddingLeft={1}>
      <Text color={isIdle ? theme.colors.dimText : theme.colors.primary}>
        {isIdle ? '●' : SPINNER_FRAMES[frameIndex]} {formatTime(elapsed)}
      </Text>
      {agentMode && (
        <Text color={theme.colors.success}> [{getAgentModeLabel()}]</Text>
      )}
      {latencyInfo !== null && (
        <Text color={getLatencyColor(latencyInfo.latencyMs)}> [{latencyInfo.modelName}:{latencyInfo.latencyMs}ms]</Text>
      )}
      <Text color={isIdle ? theme.colors.dimText : theme.colors.text}> | {getTitle()} |</Text>
    </Box>
  );
};
