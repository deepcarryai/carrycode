import React, { useMemo } from 'react';
import { Box, Text, useStdout } from 'ink';
import { Markdown } from './Markdown.js';
import { useTheme } from '../theme/index.js';
import type { ToolCallLog, ToolOperation } from '../types/index.js';
import { logger } from '../utils/logger.js';

type BodyRow = {
  kind: 'text_first' | 'text_cont' | 'tool_first' | 'tool_cont';
  text: string;
  diffFgColor?: string;
  diffBgColor?: string;
};

export interface RichAreaProps {
  content: string;
  isDone: boolean;
  color?: string;
  indent?: number;
  children?: React.ReactNode;
  title?: string;
  tools?: ToolCallLog[];
  mode?: 'text' | 'tool';
  toolOperation?: ToolOperation;
}

const ToolStageBlock = React.memo(function ToolStageBlock({
  rows,
  color,
  indent,
  titleIconWidth,
}: {
  rows: BodyRow[];
  color: string;
  indent: number;
  titleIconWidth: number;
}) {
  const { stdout } = useStdout();
  const bodyIndent = indent + titleIconWidth;
  const toolPrefixWidth = 3;
  const paddingLeft = 1; // From RichArea container
  const columns = stdout?.columns ?? 80;

  // Available width for the content area (after prefixes and indent)
  const availableWidth = Math.max(1, columns - bodyIndent - toolPrefixWidth - paddingLeft);

  const getVisualLength = (str: string) => {
    let len = 0;
    for (const char of str) {
      if (char.match(/[^\x00-\xff]/)) len += 2;
      else len += 1;
    }
    return len;
  };

  return (
    <Box flexDirection="column" marginLeft={bodyIndent}>
      {rows.map((row, idx) => {
        const finalColor = row.diffFgColor ?? color;
        const finalBgColor = row.diffBgColor;
        const displayText = row.text;

        const isFirstLine = row.kind === 'tool_first' || row.kind === 'text_first';
        const prefixChar = isFirstLine ? '└' : ' ';
        const prefix = prefixChar.padEnd(toolPrefixWidth, ' ');

        let paddedText = displayText;
        if (finalBgColor) {
          const vLen = getVisualLength(displayText || ' ');
          const linesNeeded = Math.ceil(vLen / availableWidth);
          const targetVLen = linesNeeded * availableWidth;

          let currentText = displayText || ' ';
          let currentVLen = vLen;
          while (currentVLen < targetVLen) {
            currentText += ' ';
            currentVLen += 1;
          }
          paddedText = currentText;
        }

        return (
          <Box key={`${idx}-${row.text.substring(0, 15)}`} flexDirection="row">
            <Box width={toolPrefixWidth}>
              <Text color={color} backgroundColor={finalBgColor}>{prefix}</Text>
            </Box>
            <Box flexGrow={1}>
              <Text color={finalColor} backgroundColor={finalBgColor} wrap="wrap">
                {paddedText}
              </Text>
            </Box>
          </Box>
        );
      })}
    </Box>
  );
});

export const RichArea = React.memo(({
  content,
  isDone,
  color = 'white',
  indent = 0,
  children,
  title,
  tools,
  mode = 'text',
  toolOperation,
}: RichAreaProps) => {
  const { theme } = useTheme();
  const { stdout } = useStdout();
  const bodyRows = useMemo(() => {
    const rows: BodyRow[] = [];

    const hasDiffMarkers = (lines: string[]): boolean => {
      const markerRegex = /^\s*(@@|diff |---|\+\+\+|- |\+ )/;
      return lines.some(line => markerRegex.test(line));
    };

    const diffStyleForLine = (line: string): { fg?: string; bg?: string } | undefined => {
      const t = line.trimStart();
      if (t.startsWith('+++') || t.startsWith('+')) return { fg: theme.colors.diffAddFg, bg: theme.colors.diffAddBg };
      if (t.startsWith('---') || t.startsWith('-')) return { fg: theme.colors.diffRemFg, bg: theme.colors.diffRemBg };
      if (t.startsWith('@@')) return { fg: theme.colors.info };
      return undefined;
    };

    logger.debug(JSON.stringify(tools));

    if (tools && tools.length > 0) {
      const entries = tools.map((t) => {
        let content = t.responseSummary ?? '';

        // Try to format Todo list nicely if it looks like JSON
        if (t.operation === '__TODO__' || t.toolName === 'todo_write') {
          try {
            const raw = t.responseSummary?.trim();
            if (!raw) return content;
            const parsed = JSON.parse(raw);
            const todos = (parsed && typeof parsed === 'object' ? (parsed as any).data?.todos : null) as any[] | null;
            if (!Array.isArray(todos) || todos.length === 0) return content;

            const completedCount = todos.reduce((acc, item) => acc + (item?.status === 'completed' ? 1 : 0), 0);
            const lines = [`Todo(${completedCount}/${todos.length}) -> ${todos.length} items`];
            todos.forEach((item: any, index: number) => {
              let text = item?.content || 'unknown';
              if (text.length > 30) text = text.slice(0, 30) + '...';
              const status = item?.status || 'unknown';
              const priority = item?.priority ? ` (${item.priority})` : '';
              lines.push(`${index + 1}. ${text} -> ${status}${priority}`);
            });
            return lines.join('\n');
          } catch (e) {
            // If parsing fails, just use original content
          }
        }
        return content;
      });

      const isDiffTool =
        toolOperation === '__EDITED__' ||
        String(toolOperation).includes('EDIT') ||
        tools.some(t => t.operation === '__EDITED__' || String(t.operation).includes('EDIT') || t.toolName === 'edit');

      const diffEnabled = isDiffTool || hasDiffMarkers(entries.flatMap((m) => String(m).split('\n')));

      for (const msg of entries) {
        const entryLines = String(msg).split('\n');
        const lastIndex = entryLines.length - 1;
        for (let i = 0; i < entryLines.length; i++) {
          const line = entryLines[i];
          if (i === lastIndex && line.length === 0) continue;

          const t = line.trimStart();
          const isMarkerLine = t.startsWith('+') || t.startsWith('-') || t.startsWith('@@');
          const diffStyle = (diffEnabled || isMarkerLine) ? diffStyleForLine(line) : undefined;

          rows.push({
            kind: i === 0 ? 'tool_first' : 'tool_cont',
            text: line,
            diffFgColor: diffStyle?.fg,
            diffBgColor: diffStyle?.bg,
          });
        }
      }
      return rows;
    }

    const rawLines = content.split('\n');
    while (rawLines.length > 0 && rawLines[rawLines.length - 1]?.length === 0) {
      rawLines.pop();
    }

    let start = 0;
    while (start < rawLines.length && rawLines[start]?.trim().length === 0) {
      start++;
    }
    const lines = rawLines.slice(start);

    const removeIndent = (() => {
      const nonEmpty = lines.filter((l) => l.trim().length > 0);
      if (nonEmpty.length === 0) return 0;
      const allHave4 = nonEmpty.every((l) => l.startsWith('    '));
      return allHave4 ? 4 : 0;
    })();

    const normalized = removeIndent > 0 ? lines.map((l) => (l.startsWith('    ') ? l.slice(4) : l)) : lines;

    const isDiffTool = toolOperation === '__EDITED__' || String(toolOperation).includes('EDIT') || hasDiffMarkers(normalized);
    const treatAsTool = mode === 'tool' || toolOperation != null;
    for (let i = 0; i < normalized.length; i++) {
      const text = normalized[i] ?? '';
      const t = text.trimStart();
      const isMarkerLine = t.startsWith('+') || t.startsWith('-') || t.startsWith('@@');
      const diffStyle = (isDiffTool || isMarkerLine) ? diffStyleForLine(text) : undefined;
      rows.push({
        kind: treatAsTool
          ? i === 0
            ? 'tool_first'
            : 'tool_cont'
          : i === 0
            ? 'text_first'
            : 'text_cont',
        text,
        diffFgColor: diffStyle?.fg,
        diffBgColor: diffStyle?.bg,
      });
    }

    return rows;
  }, [tools, content, mode, toolOperation]);

  const renderedContent = useMemo(() => {
    if (children) {
      return children;
    }

    const titleIconWidth = title ? 2 : 0;
    const titleHintIsTool =
      typeof title === 'string' &&
      ['Explored', 'Exploring', 'Edited', 'Editing', 'Todo', 'Bash'].includes(title.trim());
    const isTool =
      mode === 'tool' ||
      toolOperation != null ||
      (tools && tools.length > 0) ||
      titleHintIsTool ||
      bodyRows.some((r) => r.kind === 'tool_first' || r.kind === 'tool_cont');

    const finalColor = color === 'white' ? theme.colors.text : color;

    return (
      <Box flexDirection="column">
        {isTool ? (
          <ToolStageBlock
            rows={bodyRows}
            color={finalColor}
            indent={indent}
            titleIconWidth={titleIconWidth}
          />
        ) : (
          <Box marginLeft={indent + titleIconWidth}>
            <Markdown width={stdout?.columns ? Math.max(20, stdout.columns - (indent + titleIconWidth) - 2) : undefined}>
              {content}
            </Markdown>
          </Box>
        )}
      </Box>
    );
  }, [children, bodyRows, color, indent, mode, title, tools, toolOperation, theme, content, isDone, stdout]);

  return (
    <Box flexDirection="column" paddingLeft={1}>
      {title && (
        <Box flexDirection="row">
          <Box width={2}>
            <Text color={theme.colors.success}>●</Text>
          </Box>
          <Text color={theme.colors.text} bold>
            {title}
          </Text>
        </Box>
      )}
      {renderedContent}
    </Box>
  );
});
