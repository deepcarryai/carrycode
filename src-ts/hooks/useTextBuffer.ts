import { useEffect, useMemo, useState } from 'react';
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

export interface TextBuffer {
  text: string;
  isEmpty: boolean;
  viewportVisualLines: string[];
  visualCursor: VisualCursor;
  setText: (text: string) => void;
  handleKey: (key: Key) => void;
}

interface BufferState {
  lines: string[];
  cursorRow: number;
  cursorCol: number;
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
  return { lines: lines.length === 0 ? [''] : lines, cursorRow: lastRow, cursorCol: lastCol };
}

function applyInsert(state: BufferState, insertText: string): BufferState {
  const normalized = normalizeText(insertText);
  if (normalized.length === 0) return state;

  const { cursorRow, cursorCol } = state;
  const lines = [...state.lines];
  const line = lines[cursorRow] ?? '';
  const { before, after } = splitLineAtCursor(line, cursorCol);
  const parts = normalized.split('\n');

  if (parts.length === 1) {
    const nextLine = before + parts[0] + after;
    lines[cursorRow] = nextLine;
    return { lines, cursorRow, cursorCol: cursorCol + cpLen(parts[0]) };
  }

  const first = before + parts[0];
  const last = parts[parts.length - 1] + after;
  const middle = parts.slice(1, -1);

  const nextLines = [...lines.slice(0, cursorRow), first, ...middle, last, ...lines.slice(cursorRow + 1)];
  const nextRow = cursorRow + parts.length - 1;
  const nextCol = cpLen(parts[parts.length - 1] ?? '');
  return { lines: nextLines, cursorRow: nextRow, cursorCol: nextCol };
}

function applyBackspace(state: BufferState): BufferState {
  const { cursorRow, cursorCol } = state;
  const lines = [...state.lines];
  const line = lines[cursorRow] ?? '';
  const len = cpLen(line);

  if (cursorCol > 0) {
    const before = cpSlice(line, 0, cursorCol - 1);
    const after = cpSlice(line, cursorCol);
    lines[cursorRow] = before + after;
    return { lines, cursorRow, cursorCol: cursorCol - 1 };
  }

  if (cursorRow > 0) {
    const prev = lines[cursorRow - 1] ?? '';
    const merged = prev + line;
    const prevLen = cpLen(prev);
    lines.splice(cursorRow - 1, 2, merged);
    return { lines, cursorRow: cursorRow - 1, cursorCol: prevLen };
  }

  return { lines, cursorRow, cursorCol: clamp(cursorCol, 0, len) };
}

function applyDelete(state: BufferState): BufferState {
  const { cursorRow, cursorCol } = state;
  const lines = [...state.lines];
  const line = lines[cursorRow] ?? '';
  const len = cpLen(line);

  if (cursorCol < len) {
    const before = cpSlice(line, 0, cursorCol);
    const after = cpSlice(line, cursorCol + 1);
    lines[cursorRow] = before + after;
    return { lines, cursorRow, cursorCol };
  }

  if (cursorRow < lines.length - 1) {
    const next = lines[cursorRow + 1] ?? '';
    const merged = line + next;
    lines.splice(cursorRow, 2, merged);
    return { lines, cursorRow, cursorCol };
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

  const layout = useMemo(
    () => calculateLayout(state.lines, viewportWidth, state.cursorRow, state.cursorCol),
    [state.lines, state.cursorRow, state.cursorCol, viewportWidth],
  );

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
      return;
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
    if (key.name === 'tab' && key.sequence) {
      const seq = key.sequence;
      setState((s) => applyInsert(s, seq));
      return;
    }
    if (key.name === 'return') {
      if (key.shift) {
        setState((s) => applyInsert(s, '\n'));
      }
      return;
    }

    if (key.sequence && key.sequence.length > 0) {
      const seq = key.sequence;
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
  };
}
