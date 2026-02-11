import { useEffect, useMemo, useState, useRef, useCallback } from 'react';
import { Key } from './useKeypress.js';
import {
  cpLen,
  cpSlice,
  getCachedStringWidth,
  stripUnsafeCharacters,
  toCodePoints,
} from '../utils/textUtils.js';

export interface UseTextBufferProps {
  viewportWidth: number;
  viewportHeight: number;
  initialText?: string;
}

export interface VisualCursor {
  row: number;
  col: number;
}

export interface CollapsedBlock {
  id: string;
  startLine: number;
  lineCount: number;
  content: string[];
  isExpanded: boolean;
  placeholder: string;
}

export interface TextBuffer {
  text: string;
  isEmpty: boolean;
  viewportVisualLines: string[];
  visualCursor: VisualCursor;
  setText: (text: string) => void;
  handleKey: (key: Key) => void;
  cursorRow: number;
  lineCount: number;
  cursorCol: number;
  lines: string[];
  collapsedBlocks: CollapsedBlock[];
  toggleBlockAtCursor: () => void;
  expandAllBlocks: () => string;
}

interface BufferState {
  lines: string[];
  cursorRow: number;
  cursorCol: number;
  collapsedBlocks: CollapsedBlock[];
  nextBlockId: number;
}

function normalizeText(text: string): string {
  return stripUnsafeCharacters(text).replace(/\r\n/g, '\n').replace(/\r/g, '\n');
}

function clamp(n: number, min: number, max: number): number {
  if (n < min) return min;
  if (n > max) return max;
  return n;
}

function computePrefixWidths(cps: string[]): number[] {
  const prefix: number[] = new Array(cps.length + 1);
  prefix[0] = 0;
  for (let i = 0; i < cps.length; i++) {
    prefix[i + 1] = prefix[i] + getCachedStringWidth(cps[i]);
  }
  return prefix;
}

function findLastSpace(cps: string[], start: number, endExclusive: number): number {
  for (let i = endExclusive - 1; i >= start; i--) {
    if (cps[i] === ' ') return i;
  }
  return -1;
}

function wrapLogicalLine(
  logicalLine: string,
  viewportWidth: number,
): { segments: string[]; segmentStarts: number[] } {
  const maxWidth = Math.max(1, viewportWidth);
  const cps = toCodePoints(logicalLine);
  if (cps.length === 0) return { segments: [''], segmentStarts: [0] };

  const prefix = computePrefixWidths(cps);
  const segments: string[] = [];
  const segmentStarts: number[] = [];

  let start = 0;
  while (start < cps.length) {
    let end = start;
    while (end < cps.length) {
      const w = prefix[end + 1] - prefix[start];
      if (w > maxWidth) break;
      end++;
    }

    if (end >= cps.length) {
      segmentStarts.push(start);
      segments.push(cps.slice(start).join(''));
      break;
    }

    if (end === start) {
      segmentStarts.push(start);
      segments.push(cps.slice(start, start + 1).join(''));
      start = start + 1;
      continue;
    }

    const lastSpace = findLastSpace(cps, start, end);
    if (lastSpace >= start) {
      segmentStarts.push(start);
      segments.push(cps.slice(start, lastSpace).join(''));
      start = lastSpace + 1;
      continue;
    }

    segmentStarts.push(start);
    segments.push(cps.slice(start, end).join(''));
    start = end;
  }

  return { segments, segmentStarts };
}

function calculateLayout(
  logicalLines: string[],
  viewportWidth: number,
  cursorRow: number,
  cursorCol: number,
): { visualLines: string[]; cursor: VisualCursor; visualRowForLogicalRow: number[] } {
  const visualLines: string[] = [];
  const visualRowForLogicalRow: number[] = [];

  let visualCursorRow = 0;
  let visualCursorCol = 0;

  for (let row = 0; row < logicalLines.length; row++) {
    visualRowForLogicalRow[row] = visualLines.length;

    const logicalLine = logicalLines[row];
    const cps = toCodePoints(logicalLine);
    const prefix = computePrefixWidths(cps);

    const { segments, segmentStarts } = wrapLogicalLine(logicalLine, viewportWidth);
    for (let segIndex = 0; segIndex < segments.length; segIndex++) {
      visualLines.push(segments[segIndex]);
    }

    if (row === cursorRow) {
      const clampedCursorCol = clamp(cursorCol, 0, cps.length);

      let segIndex = 0;
      while (
        segIndex + 1 < segmentStarts.length &&
        segmentStarts[segIndex + 1] <= clampedCursorCol
      ) {
        segIndex++;
      }

      const segStart = segmentStarts[segIndex] ?? 0;
      const colWidth = prefix[clampedCursorCol] - prefix[segStart];

      visualCursorRow = (visualRowForLogicalRow[row] ?? 0) + segIndex;
      visualCursorCol = colWidth;
    }
  }

  if (logicalLines.length === 0) {
    return { visualLines: [''], cursor: { row: 0, col: 0 }, visualRowForLogicalRow: [0] };
  }

  return {
    visualLines: visualLines.length === 0 ? [''] : visualLines,
    cursor: { row: visualCursorRow, col: visualCursorCol },
    visualRowForLogicalRow,
  };
}

function splitLineAtCursor(line: string, cursorCol: number): { before: string; after: string } {
  return {
    before: cpSlice(line, 0, cursorCol),
    after: cpSlice(line, cursorCol),
  };
}

function setTextState(text: string): BufferState {
  const normalized = normalizeText(text);
  const lines = normalized.split('\n');
  const lastRow = Math.max(0, lines.length - 1);
  const lastCol = cpLen(lines[lastRow] ?? '');
  return {
    lines: lines.length === 0 ? [''] : lines,
    cursorRow: lastRow,
    cursorCol: lastCol,
    collapsedBlocks: [],
    nextBlockId: 1,
  };
}

function applyInsert(state: BufferState, insertText: string): BufferState {
  // 如果光标在折叠块内部，阻止编辑
  if (isInsideCollapsedBlock(state)) {
    return state;
  }

  const normalized = normalizeText(insertText);
  if (normalized.length === 0) return state;

  const parts = normalized.split('\n');
  const COLLAPSE_THRESHOLD = 5;

  // 检测大段粘贴并自动折叠
  if (parts.length > COLLAPSE_THRESHOLD) {
    return applyCollapsedInsert(state, parts);
  }

  const { cursorRow, cursorCol } = state;
  const lines = [...state.lines];
  const line = lines[cursorRow] ?? '';
  const { before, after } = splitLineAtCursor(line, cursorCol);

  if (parts.length === 1) {
    const nextLine = before + parts[0] + after;
    lines[cursorRow] = nextLine;
    return { ...state, lines, cursorCol: cursorCol + cpLen(parts[0]) };
  }

  const first = before + parts[0];
  const last = parts[parts.length - 1] + after;
  const middle = parts.slice(1, -1);

  const nextLines = [...lines.slice(0, cursorRow), first, ...middle, last, ...lines.slice(cursorRow + 1)];
  const nextRow = cursorRow + parts.length - 1;
  const nextCol = cpLen(parts[parts.length - 1] ?? '');
  return { ...state, lines: nextLines, cursorRow: nextRow, cursorCol: nextCol };
}

function applyCollapsedInsert(state: BufferState, insertLines: string[]): BufferState {
  const blockId = `paste-${state.nextBlockId}`;
  const lineCount = insertLines.length;

  const block: CollapsedBlock = {
    id: blockId,
    startLine: state.cursorRow,
    lineCount,
    content: insertLines,
    isExpanded: false,
    placeholder: `[Pasted text #${state.nextBlockId} +${lineCount} lines]`,
  };

  const currentLine = state.lines[state.cursorRow] ?? '';
  const { before, after } = splitLineAtCursor(currentLine, state.cursorCol);

  const newLines = [...state.lines];
  newLines[state.cursorRow] = before + block.placeholder + after;

  return {
    ...state,
    lines: newLines,
    collapsedBlocks: [...state.collapsedBlocks, block],
    nextBlockId: state.nextBlockId + 1,
    cursorCol: state.cursorCol + cpLen(block.placeholder),
  };
}

function toggleBlockAtCursor(state: BufferState): BufferState {
  const currentLine = state.lines[state.cursorRow] ?? '';

  // 查找当前行是否包含折叠块占位符
  const block = state.collapsedBlocks.find(
    (b) => b.startLine === state.cursorRow && currentLine.includes(b.placeholder),
  );

  if (!block) return state;

  if (block.isExpanded) {
    return collapseBlock(state, block);
  } else {
    return expandBlock(state, block);
  }
}

function expandBlock(state: BufferState, block: CollapsedBlock): BufferState {
  const currentLine = state.lines[state.cursorRow];
  const placeholderIndex = currentLine.indexOf(block.placeholder);

  if (placeholderIndex === -1) return state;

  const before = currentLine.slice(0, placeholderIndex);
  const after = currentLine.slice(placeholderIndex + block.placeholder.length);

  // 构建展开后的行
  const expandedLines = block.content.map((line, i) => {
    if (i === 0) return before + line;
    if (i === block.content.length - 1) return line + after;
    return line;
  });

  const newLines = [
    ...state.lines.slice(0, state.cursorRow),
    ...expandedLines,
    ...state.lines.slice(state.cursorRow + 1),
  ];

  // 更新折叠块状态
  const updatedBlocks = state.collapsedBlocks.map((b) =>
    b.id === block.id ? { ...b, isExpanded: true, startLine: state.cursorRow } : b,
  );

  // 调整光标位置：如果光标在占位符位置，移动到展开内容的开始
  let newCursorRow = state.cursorRow;
  let newCursorCol = state.cursorCol;

  if (state.cursorCol >= placeholderIndex && state.cursorCol <= placeholderIndex + cpLen(block.placeholder)) {
    // 光标在占位符内或边界，移动到第一行展开内容的开始
    newCursorCol = cpLen(before);
  }

  return {
    ...state,
    lines: newLines,
    collapsedBlocks: updatedBlocks,
    cursorRow: newCursorRow,
    cursorCol: newCursorCol,
  };
}

function collapseBlock(state: BufferState, block: CollapsedBlock): BufferState {
  // 将展开的多行内容替换回占位符
  const startLine = block.startLine;
  const endLine = startLine + block.lineCount - 1;

  const firstLine = state.lines[startLine] || '';
  const lastLine = state.lines[endLine] || '';

  // 提取占位符前后的内容
  const firstContentIndex = firstLine.indexOf(block.content[0]);
  const before = firstContentIndex >= 0 ? firstLine.slice(0, firstContentIndex) : '';

  const lastContent = block.content[block.content.length - 1];
  const lastContentIndex = lastLine.indexOf(lastContent);
  const after =
    lastContentIndex >= 0 ? lastLine.slice(lastContentIndex + lastContent.length) : '';

  const collapsedLine = before + block.placeholder + after;

  const newLines = [
    ...state.lines.slice(0, startLine),
    collapsedLine,
    ...state.lines.slice(endLine + 1),
  ];

  const updatedBlocks = state.collapsedBlocks.map((b) =>
    b.id === block.id ? { ...b, isExpanded: false } : b,
  );

  // 调整光标位置
  let newCursorRow = state.cursorRow;
  let newCursorCol = state.cursorCol;

  if (state.cursorRow >= startLine && state.cursorRow <= endLine) {
    newCursorRow = startLine;
    newCursorCol = cpLen(before + block.placeholder);
  } else if (state.cursorRow > endLine) {
    newCursorRow = state.cursorRow - (endLine - startLine);
  }

  return {
    ...state,
    lines: newLines,
    collapsedBlocks: updatedBlocks,
    cursorRow: newCursorRow,
    cursorCol: newCursorCol,
  };
}

function expandAllBlocks(state: BufferState): BufferState {
  let currentState = state;

  // 从后往前展开，避免行号变化影响
  const blocksToExpand = [...state.collapsedBlocks]
    .filter((b) => !b.isExpanded)
    .sort((a, b) => b.startLine - a.startLine);

  for (const block of blocksToExpand) {
    currentState = expandBlock(currentState, block);
  }

  return currentState;
}

function getExpandedText(state: BufferState): string {
  const expandedState = expandAllBlocks(state);
  return expandedState.lines.join('\n');
}

function isInsideCollapsedBlock(state: BufferState): boolean {
  const currentLine = state.lines[state.cursorRow] ?? '';
  const block = state.collapsedBlocks.find(
    (b) => b.startLine === state.cursorRow && !b.isExpanded && currentLine.includes(b.placeholder),
  );

  if (!block) return false;

  const placeholderIndex = currentLine.indexOf(block.placeholder);
  if (placeholderIndex === -1) return false;

  const placeholderEnd = placeholderIndex + cpLen(block.placeholder);
  return state.cursorCol > placeholderIndex && state.cursorCol < placeholderEnd;
}

function deleteCollapsedBlock(state: BufferState): BufferState {
  const currentLine = state.lines[state.cursorRow] ?? '';
  const block = state.collapsedBlocks.find(
    (b) => b.startLine === state.cursorRow && !b.isExpanded && currentLine.includes(b.placeholder),
  );

  if (!block) return state;

  const placeholderIndex = currentLine.indexOf(block.placeholder);
  if (placeholderIndex === -1) return state;

  // 删除整个折叠块占位符
  const before = currentLine.slice(0, placeholderIndex);
  const after = currentLine.slice(placeholderIndex + block.placeholder.length);
  const newLine = before + after;

  const newLines = [...state.lines];
  newLines[state.cursorRow] = newLine;

  // 从折叠块列表中移除
  const newBlocks = state.collapsedBlocks.filter((b) => b.id !== block.id);

  return {
    ...state,
    lines: newLines,
    collapsedBlocks: newBlocks,
    cursorCol: placeholderIndex,
  };
}

function applyBackspace(state: BufferState): BufferState {
  // 如果光标在折叠块内部，阻止编辑
  if (isInsideCollapsedBlock(state)) {
    return state;
  }

  // 检查是否要删除折叠块占位符
  const currentLine = state.lines[state.cursorRow] ?? '';
  const block = state.collapsedBlocks.find(
    (b) => b.startLine === state.cursorRow && !b.isExpanded && currentLine.includes(b.placeholder),
  );

  if (block) {
    const placeholderIndex = currentLine.indexOf(block.placeholder);
    const placeholderEnd = placeholderIndex + cpLen(block.placeholder);

    // 如果光标在占位符结束位置，删除整个折叠块
    if (state.cursorCol === placeholderEnd) {
      return deleteCollapsedBlock(state);
    }
  }

  const { cursorRow, cursorCol } = state;
  const lines = [...state.lines];
  const line = lines[cursorRow] ?? '';
  const len = cpLen(line);

  if (cursorCol > 0) {
    const before = cpSlice(line, 0, cursorCol - 1);
    const after = cpSlice(line, cursorCol);
    lines[cursorRow] = before + after;
    return { ...state, lines, cursorCol: cursorCol - 1 };
  }

  if (cursorRow > 0) {
    const prev = lines[cursorRow - 1] ?? '';
    const merged = prev + line;
    const prevLen = cpLen(prev);
    lines.splice(cursorRow - 1, 2, merged);
    return { ...state, lines, cursorRow: cursorRow - 1, cursorCol: prevLen };
  }

  return { ...state, lines, cursorCol: clamp(cursorCol, 0, len) };
}

function applyDelete(state: BufferState): BufferState {
  // 如果光标在折叠块内部，阻止编辑
  if (isInsideCollapsedBlock(state)) {
    return state;
  }

  // 检查是否要删除折叠块占位符
  const currentLine = state.lines[state.cursorRow] ?? '';
  const block = state.collapsedBlocks.find(
    (b) => b.startLine === state.cursorRow && !b.isExpanded && currentLine.includes(b.placeholder),
  );

  if (block) {
    const placeholderIndex = currentLine.indexOf(block.placeholder);

    // 如果光标在占位符开始位置，删除整个折叠块
    if (state.cursorCol === placeholderIndex) {
      return deleteCollapsedBlock(state);
    }
  }

  const { cursorRow, cursorCol } = state;
  const lines = [...state.lines];
  const line = lines[cursorRow] ?? '';
  const len = cpLen(line);

  if (cursorCol < len) {
    const before = cpSlice(line, 0, cursorCol);
    const after = cpSlice(line, cursorCol + 1);
    lines[cursorRow] = before + after;
    return { ...state, lines };
  }

  if (cursorRow < lines.length - 1) {
    const next = lines[cursorRow + 1] ?? '';
    const merged = line + next;
    lines.splice(cursorRow, 2, merged);
    return { ...state, lines };
  }

  return state;
}

function moveCursor(state: BufferState, deltaRow: number, deltaCol: number): BufferState {
  const nextRow = clamp(state.cursorRow + deltaRow, 0, state.lines.length - 1);
  const nextLineLen = cpLen(state.lines[nextRow] ?? '');
  const nextCol = clamp(state.cursorCol + deltaCol, 0, nextLineLen);
  return { ...state, cursorRow: nextRow, cursorCol: nextCol };
}

function moveCursorToCol(state: BufferState, col: number): BufferState {
  const lineLen = cpLen(state.lines[state.cursorRow] ?? '');
  return { ...state, cursorCol: clamp(col, 0, lineLen) };
}

export function useTextBuffer({
  viewportWidth,
  viewportHeight,
  initialText = '',
}: UseTextBufferProps): TextBuffer {
  const [state, setState] = useState<BufferState>(() => setTextState(initialText));
  const [scrollRow, setScrollRow] = useState(0);

  // 粘贴缓冲区：用于合并连续的粘贴事件
  const pasteBufferRef = useRef<string[]>([]);
  const pasteTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const pasteCursorRef = useRef<{ row: number; col: number } | null>(null);

  const layout = useMemo(
    () => calculateLayout(state.lines, viewportWidth, state.cursorRow, state.cursorCol),
    [state.lines, state.cursorRow, state.cursorCol, viewportWidth],
  );

  // 清理定时器
  useEffect(() => {
    return () => {
      if (pasteTimeoutRef.current) {
        clearTimeout(pasteTimeoutRef.current);
      }
    };
  }, []);

  useEffect(() => {
    const cursorRow = layout.cursor.row;
    const maxScroll = Math.max(0, layout.visualLines.length - viewportHeight);
    let nextScroll = scrollRow;

    if (cursorRow < scrollRow) {
      nextScroll = cursorRow;
    } else if (cursorRow >= scrollRow + viewportHeight) {
      nextScroll = cursorRow - viewportHeight + 1;
    }

    nextScroll = clamp(nextScroll, 0, maxScroll);
    if (nextScroll !== scrollRow) setScrollRow(nextScroll);
  }, [layout.cursor.row, layout.visualLines.length, scrollRow, viewportHeight]);

  const viewportVisualLines = useMemo(() => {
    const slice = layout.visualLines.slice(scrollRow, scrollRow + viewportHeight);
    if (slice.length >= viewportHeight) return slice;
    return [...slice, ...new Array(viewportHeight - slice.length).fill('')];
  }, [layout.visualLines, scrollRow, viewportHeight]);

  const visualCursor: VisualCursor = useMemo(() => {
    const row = clamp(layout.cursor.row - scrollRow, 0, viewportHeight - 1);
    const col = clamp(layout.cursor.col, 0, Math.max(0, viewportWidth));
    return { row, col };
  }, [layout.cursor.row, layout.cursor.col, scrollRow, viewportHeight, viewportWidth]);

  const text = state.lines.join('\n');
  const isEmpty = text.length === 0;

  function setText(nextText: string) {
    setState(setTextState(nextText));
    setScrollRow(0);
  }

  function handleKey(key: Key) {
    if (key.ctrl || key.meta) {
      if (key.name !== 'return') {
        return;
      }
    }

    if (key.name === 'left') {
      setState((s) => moveCursor(s, 0, -1));
      return;
    }
    if (key.name === 'right') {
      setState((s) => moveCursor(s, 0, 1));
      return;
    }
    if (key.name === 'up') {
      setState((s) => moveCursor(s, -1, 0));
      return;
    }
    if (key.name === 'down') {
      setState((s) => moveCursor(s, 1, 0));
      return;
    }
    if (key.name === 'backspace') {
      setState((s) => applyBackspace(s));
      return;
    }
    if (key.name === 'delete') {
      setState((s) => applyDelete(s));
      return;
    }
    if (key.name === 'tab') {
      // Tab 键用于展开/折叠当前行的折叠块
      setState((s) => {
        const currentLine = s.lines[s.cursorRow] ?? '';
        const hasCollapsedBlock = s.collapsedBlocks.some(
          (b) => b.startLine === s.cursorRow && currentLine.includes(b.placeholder),
        );

        if (hasCollapsedBlock) {
          return toggleBlockAtCursor(s);
        }

        // 如果没有折叠块，插入 tab 字符
        if (key.sequence) {
          return applyInsert(s, key.sequence);
        }
        return s;
      });
      return;
    }
    if (key.name === 'return') {
      if (key.shift || key.ctrl || key.meta) {
        setState((s) => applyInsert(s, '\n'));
      }
      return;
    }

    if (key.sequence && key.sequence.length > 0) {
      const seq = key.sequence;

      // 先规范化文本，将 \r\n 和 \r 转换为 \n
      const normalized = normalizeText(seq);
      const parts = normalized.split('\n');

      // 检测是否为多行输入（可能是粘贴）
      if (parts.length > 1) {

        // 清除之前的定时器
        if (pasteTimeoutRef.current) {
          clearTimeout(pasteTimeoutRef.current);
        }

        // 如果是新的粘贴操作（光标位置变化或缓冲区为空）
        const isNewPaste =
          pasteBufferRef.current.length === 0 ||
          !pasteCursorRef.current ||
          pasteCursorRef.current.row !== state.cursorRow ||
          pasteCursorRef.current.col !== state.cursorCol;

        if (isNewPaste) {
          pasteBufferRef.current = parts;
          pasteCursorRef.current = { row: state.cursorRow, col: state.cursorCol };
        } else {
          // 合并到现有缓冲区
          pasteBufferRef.current = [...pasteBufferRef.current, ...parts];
        }

        // 设置定时器：150ms 内没有新输入，则处理缓冲区
        pasteTimeoutRef.current = setTimeout(() => {
          const allParts = pasteBufferRef.current;
          const totalLines = allParts.length;

          if (totalLines === 0) return;

          setState((s) => {
            // 使用保存的光标位置
            const startCursor = pasteCursorRef.current!;

            // 应用完整的粘贴内容
            const normalized = normalizeText(allParts.join('\n'));

            // 创建临时状态，使用粘贴开始时的光标位置
            const tempState = {
              ...s,
              cursorRow: startCursor.row,
              cursorCol: startCursor.col,
            };

            const result = applyInsert(tempState, normalized);

            // 清空缓冲区
            pasteBufferRef.current = [];
            pasteCursorRef.current = null;

            return result;
          });
        }, 150);

        // 暂时不更新状态，等待合并
        return;
      }

      // 单行输入，直接处理
      setState((s) => applyInsert(s, seq));
    }
  }

  return {
    text,
    isEmpty,
    viewportVisualLines,
    visualCursor,
    setText,
    handleKey,
    cursorRow: state.cursorRow,
    lineCount: state.lines.length,
    cursorCol: state.cursorCol,
    lines: state.lines,
    collapsedBlocks: state.collapsedBlocks,
    toggleBlockAtCursor: () => setState((s) => toggleBlockAtCursor(s)),
    expandAllBlocks: () => getExpandedText(state),
  };
}
