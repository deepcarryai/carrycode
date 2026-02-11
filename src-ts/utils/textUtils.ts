const codePointsCache = new Map<string, string[]>();
const stringWidthCache = new Map<string, number>();
const MAX_STRING_LENGTH_TO_CACHE = 1000;

function isAscii(str: string): boolean {
  for (let i = 0; i < str.length; i++) {
    if (str.charCodeAt(i) > 127) return false;
  }
  return true;
}

export function toCodePoints(str: string): string[] {
  if (isAscii(str)) return str.split('');

  if (str.length <= MAX_STRING_LENGTH_TO_CACHE) {
    const cached = codePointsCache.get(str);
    if (cached) return cached;
  }

  const cps = Array.from(str);
  if (str.length <= MAX_STRING_LENGTH_TO_CACHE) codePointsCache.set(str, cps);
  return cps;
}

export function cpLen(str: string): number {
  return toCodePoints(str).length;
}

export function cpSlice(str: string, start: number, end?: number): string {
  return toCodePoints(str).slice(start, end).join('');
}

export function stripUnsafeCharacters(str: string): string {
  const cps = toCodePoints(str);
  const kept: string[] = [];

  for (const ch of cps) {
    const code = ch.codePointAt(0);
    if (code === undefined) continue;
    if (code === 0x0a || code === 0x0d || code === 0x09) {
      kept.push(ch);
      continue;
    }
    if ((code >= 0x00 && code <= 0x1f) || (code >= 0x80 && code <= 0x9f) || code === 0x7f) {
      continue;
    }
    kept.push(ch);
  }

  return kept.join('');
}

function isCombining(code: number): boolean {
  return (
    (code >= 0x0300 && code <= 0x036f) ||
    (code >= 0x1ab0 && code <= 0x1aff) ||
    (code >= 0x1dc0 && code <= 0x1dff) ||
    (code >= 0x20d0 && code <= 0x20ff) ||
    (code >= 0xfe20 && code <= 0xfe2f)
  );
}

function isWide(code: number): boolean {
  return (
    (code >= 0x1100 && code <= 0x115f) ||
    (code >= 0x2329 && code <= 0x232a) ||
    (code >= 0x2e80 && code <= 0xa4cf) ||
    (code >= 0xac00 && code <= 0xd7a3) ||
    (code >= 0xf900 && code <= 0xfaff) ||
    (code >= 0xfe10 && code <= 0xfe19) ||
    (code >= 0xfe30 && code <= 0xfe6f) ||
    (code >= 0xff00 && code <= 0xff60) ||
    (code >= 0xffe0 && code <= 0xffe6) ||
    (code >= 0x1f300 && code <= 0x1f64f) ||
    (code >= 0x1f900 && code <= 0x1f9ff) ||
    (code >= 0x20000 && code <= 0x3fffd)
  );
}

function charWidth(ch: string): number {
  const code = ch.codePointAt(0);
  if (code === undefined) return 0;
  if (code === 0x200d) return 0;
  if (code >= 0xfe00 && code <= 0xfe0f) return 0;
  if (isCombining(code)) return 0;
  if (isWide(code)) return 2;
  return 1;
}

export function getCachedStringWidth(str: string): number {
  if (/^[\x20-\x7E]*$/.test(str)) return str.length;

  const cached = stringWidthCache.get(str);
  if (cached !== undefined) return cached;

  const width = toCodePoints(str).reduce((sum, ch) => sum + charWidth(ch), 0);
  stringWidthCache.set(str, width);
  return width;
}

export function clearStringWidthCache(): void {
  stringWidthCache.clear();
}

export function truncateToWidth(str: string, maxWidth: number): string {
  if (maxWidth <= 0) return '';
  if (getCachedStringWidth(str) <= maxWidth) return str;

  let out = '';
  let width = 0;
  for (const ch of toCodePoints(str)) {
    const cw = charWidth(ch);
    if (cw <= 0) {
      out += ch;
      continue;
    }
    if (width + cw > maxWidth) break;
    out += ch;
    width += cw;
    if (width === maxWidth) break;
  }
  return out;
}

export function truncateToWidthWithEllipsis(
  str: string,
  maxWidth: number,
  ellipsis: string = '…',
): string {
  if (maxWidth <= 0) return '';

  const currentWidth = getCachedStringWidth(str);
  if (currentWidth <= maxWidth) return str;

  const ellipsisWidth = getCachedStringWidth(ellipsis);
  if (ellipsisWidth <= 0 || ellipsisWidth >= maxWidth) {
    return truncateToWidth(str, maxWidth);
  }

  return truncateToWidth(str, maxWidth - ellipsisWidth) + ellipsis;
}

export function padRightToWidth(str: string, targetWidth: number): string {
  const currentWidth = getCachedStringWidth(str);
  if (currentWidth >= targetWidth) return str;
  return str + ' '.repeat(targetWidth - currentWidth);
}

export function padLeftToWidth(str: string, targetWidth: number): string {
  const currentWidth = getCachedStringWidth(str);
  if (currentWidth >= targetWidth) return str;
  return ' '.repeat(targetWidth - currentWidth) + str;
}

export function backspaceByGraphemeApprox(str: string): string {
  const cps = toCodePoints(str);
  if (cps.length === 0) return str;

  const pop = () => cps.pop();

  const isZeroWidth = (code: number): boolean => {
    if (code === 0x200d) return true;
    if (code >= 0xfe00 && code <= 0xfe0f) return true;
    return isCombining(code);
  };

  let popped = pop();
  if (!popped) return '';

  let removedZw = popped.codePointAt(0) === 0x200d;

  while (cps.length > 0) {
    const last = cps[cps.length - 1]!;
    const code = last.codePointAt(0);
    if (code === undefined) {
      pop();
      continue;
    }

    if (isZeroWidth(code)) {
      removedZw = removedZw || code === 0x200d;
      pop();
      continue;
    }

    if (removedZw) {
      pop();
      removedZw = false;
      continue;
    }

    break;
  }

  return cps.join('');
}

/**
 * Strip ANSI escape codes from a string to calculate display width
 */
export function stripAnsiCodes(str: string): string {
  return str.replace(/\x1b\[[0-9;]*m/g, '');
}

/**
 * Get string display width, ignoring ANSI escape codes
 */
export function getStringWidthIgnoringAnsi(str: string): number {
  const stripped = stripAnsiCodes(str);
  return getCachedStringWidth(stripped);
}
