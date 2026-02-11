import React, { useMemo } from 'react';
import { Box, Text, useStdout } from 'ink';
import { Markdown } from './Markdown.js';
import { useTheme } from '../theme/index.js';
import type { ToolCallLog, ToolOperation } from '../types/index.js';
import { logger } from '../utils/logger.js';
import { getCachedStringWidth, padRightToWidth } from '../utils/textUtils.js';

type BodyRow = {
  kind: 'text_first' | 'text_cont' | 'tool_first' | 'tool_cont';
  text: string;
  diffFgColor?: string;
  diffBgColor?: string;
  lineNo?: string;
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
  const { theme } = useTheme();
  const bodyIndent = indent + titleIconWidth;
  const toolPrefixWidth = 2;
  const paddingLeft = 1; // From RichArea container
  const columns = stdout?.columns ?? 80;

  // Calculate max line number width for diffs
  const maxLineNoWidth = useMemo(() => {
    let max = 0;
    rows.forEach(row => {
      if (row.lineNo) {
        max = Math.max(max, row.lineNo.length);
      }
    });
    return max > 0 ? max + 1 : 0; // +1 for a bit of padding
  }, [rows]);

  // Available width for the content area (after prefixes, indent, and line numbers)
  const availableWidth = Math.max(1, columns - bodyIndent - toolPrefixWidth - paddingLeft - maxLineNoWidth);

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
          const vLen = getCachedStringWidth(displayText || ' ');
          const linesNeeded = Math.ceil(vLen / availableWidth);
          const targetVLen = linesNeeded * availableWidth;
          paddedText = padRightToWidth(displayText || ' ', targetVLen);
        }

        return (
          <Box key={`${idx}-${row.text.substring(0, 15)}`} flexDirection="row">
            <Box width={toolPrefixWidth} flexShrink={0}>
              <Text color={color} backgroundColor={finalBgColor}>{prefix}</Text>
            </Box>
            {maxLineNoWidth > 0 && (
              <Box width={maxLineNoWidth} flexShrink={0}>
                <Text color={theme.colors.info} backgroundColor={finalBgColor}>
                  {(row.lineNo || '').padStart(maxLineNoWidth - 1, ' ') + ' '}
                </Text>
              </Box>
            )}
            <Box width={availableWidth}>
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
      // Support traditional unified diff format
      const markerRegex = /^\s*(@@|diff |---|\+\+\+|- |\+ )/;
      if (lines.some(line => markerRegex.test(line))) {
        return true;
      }
      
      // Support line number format (e.g., "15 -", "16 +")
      const lineNumRegex = /^\s*\d+\s+[-+]/;
      return lines.some(line => lineNumRegex.test(line));
    };

    const diffStyleForLine = (line: string): { fg?: string; bg?: string } | undefined => {
      const t = line.trimStart();
      
      // Check for line number prefix: /^\d+\s+[-+ ]/
      const lineNumMatch = t.match(/^(\d+)\s+([-+ ])/);
      if (lineNumMatch) {
        const operator = lineNumMatch[2];
        if (operator === '+') {
          return { fg: theme.colors.diffAddFg, bg: theme.colors.diffAddBg };
        }
        if (operator === '-') {
          return { fg: theme.colors.diffRemFg, bg: theme.colors.diffRemBg };
        }
        // Context line (space operator)
        return undefined;
      }
      
      // Fallback for traditional diff format
      if (t.startsWith('diff ') || t.startsWith('@@') || t.startsWith('+++') || t.startsWith('---')) {
        return { fg: theme.colors.info };
      }
      if (t.startsWith('+')) return { fg: theme.colors.diffAddFg, bg: theme.colors.diffAddBg };
      if (t.startsWith('-')) return { fg: theme.colors.diffRemFg, bg: theme.colors.diffRemBg };
      return undefined;
    };

    logger.debug(JSON.stringify(tools));

    if (tools && tools.length > 0) {
      const entries = tools.map((t) => {
        let content = t.responseSummary ?? '';

        // Try to format Todo list nicely if it looks like JSON
        if (t.operation === '__TODO__' || t.toolName === 'todo_write' || t.toolName === 'core_todo_write') {
          try {
            const raw = t.responseSummary?.trim();
            if (!raw) return content;
            const parsed = JSON.parse(raw);

            // The JSON from Rust can be the raw tool result or the parsed data
            let todos = (parsed && typeof parsed === 'object' ? (parsed as any).data?.todos : null) as any[] | null;
            if (!todos && Array.isArray(parsed)) {
              todos = parsed;
            } else if (!todos && parsed && typeof parsed === 'object' && Array.isArray((parsed as any).todos)) {
              todos = (parsed as any).todos;
            }

            if (!Array.isArray(todos) || todos.length === 0) return content;

            const completedCount = todos.reduce((acc, item) => acc + (item?.status === 'completed' ? 1 : 0), 0);
            const lines = [`Todo(${completedCount}/${todos.length}) -> ${todos.length} items`];
            todos.forEach((item: any, index: number) => {
              const text = item?.content || 'unknown';
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
        tools.some(t => t.operation === '__EDITED__' || String(t.operation).includes('EDIT') || t.toolName === 'edit' || t.toolName === 'core_edit');

      const diffEnabled = isDiffTool || hasDiffMarkers(entries.flatMap((m) => String(m).split(/\r?\n/)));

      // Diff line number tracking
      let oldLine = 0;
      let newLine = 0;
      let inDiffHunk = false;

      for (const msg of entries) {
        const entryLines = String(msg).split(/\r?\n/);
        const lastIndex = entryLines.length - 1;
        for (let i = 0; i < entryLines.length; i++) {
          const line = entryLines[i];
          if (i === lastIndex && line.length === 0) continue;

          const t = line.trimStart();
          const isMarkerLine = t.startsWith('+') || t.startsWith('-') || t.startsWith('@@') || /^\d+\s+[-+]/.test(t);
          const diffStyle = (diffEnabled || isMarkerLine) ? diffStyleForLine(line) : undefined;

          let lineNo: string | undefined;
          if (diffEnabled || isMarkerLine) {
            // Try to parse hunk header
            const hunkMatch = line.match(/^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/);
            if (hunkMatch) {
              oldLine = parseInt(hunkMatch[1], 10);
              newLine = parseInt(hunkMatch[2], 10);
              inDiffHunk = true;
            } else if (inDiffHunk) {
              if (line.startsWith(' ') || line.startsWith('  ')) { // Context line (standard or indented)
                lineNo = newLine.toString();
                oldLine++;
                newLine++;
              } else if (line.startsWith('-')) { // Deletion
                lineNo = oldLine.toString();
                oldLine++;
              } else if (line.startsWith('+')) { // Insertion
                lineNo = newLine.toString();
                newLine++;
              } else if (line.startsWith('diff --git') || line.startsWith('--- ') || line.startsWith('+++ ')) {
                // Reset for new file or header
                inDiffHunk = false;
              }
            }
          }

          rows.push({
            kind: i === 0 ? 'tool_first' : 'tool_cont',
            text: line,
            diffFgColor: diffStyle?.fg,
            diffBgColor: diffStyle?.bg,
            lineNo,
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
      let text = normalized[i] ?? '';
      const t = text.trim();

      // Intercept and format agent_tool_calls_json in stream content
      if (text.includes('<agent_tool_calls_json>')) {
        try {
          const startTag = '<agent_tool_calls_json>';
          const endTag = '</agent_tool_calls_json>';

          let fullBlock = text;
          let endIndex = i;

          // If the closing tag isn't on this line, look ahead
          if (!text.includes(endTag)) {
            for (let j = i + 1; j < normalized.length; j++) {
              const nextLine = normalized[j] || '';
              fullBlock += '\n' + nextLine;
              if (nextLine.includes(endTag)) {
                endIndex = j;
                break;
              }
            }
          }

          const startIdx = fullBlock.indexOf(startTag);
          const endIdx = fullBlock.indexOf(endTag);

          if (startIdx !== -1) {
            let jsonStr = '';
            if (endIdx !== -1) {
              jsonStr = fullBlock.slice(startIdx + startTag.length, endIdx).trim();
            } else {
              // Partial block during stream
              jsonStr = fullBlock.slice(startIdx + startTag.length).trim();
            }

            if (jsonStr) {
              try {
                const parsed = JSON.parse(jsonStr);
                const calls = Array.isArray(parsed) ? parsed : [parsed];

                for (const call of calls) {
                  const name = call.name || 'unknown';
                  if (name === 'core_todo_write' || name === 'todo_write') {
                    const args = typeof call.arguments === 'string' ? JSON.parse(call.arguments) : (call.arguments || {});
                    const todos = Array.isArray(args.todos) ? args.todos : [];
                    const completedCount = todos.filter((td: any) => td?.status === 'completed').length;

                    rows.push({
                      kind: 'tool_first',
                      text: `Todo(${completedCount}/${todos.length}) -> ${todos.length} items`,
                    });
                    for (let k = 0; k < todos.length; k++) {
                      const todo = todos[k];
                      rows.push({
                        kind: 'tool_cont',
                        text: `${k + 1}. ${todo.content || ''} -> ${todo.status || 'pending'}`,
                      });
                    }
                  } else {
                    rows.push({ kind: 'tool_first', text: `- ${name}` });
                  }
                }
              } catch (e) {
                // If JSON is incomplete during streaming, just show a placeholder or nothing
                rows.push({ kind: 'tool_first', text: `... processing tools ...` });
              }

              // Skip the lines we've consumed
              i = endIndex;
              continue;
            }
          }
        } catch (e) {
          // Fallback
        }
      }

      const t_start = text.trimStart();
      const isMarkerLine = t_start.startsWith('+') || t_start.startsWith('-') || t_start.startsWith('@@');
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
