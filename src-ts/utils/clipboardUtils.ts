export function escapePath(path: string): string {
  return path.replace(/ /g, '\\ ');
}

export function unescapePath(path: string): string {
  return path.replace(/\\ /g, ' ');
}

const PATH_PREFIX_PATTERN = /^([/~.]|[a-zA-Z]:|\\\\)/;

export function splitEscapedPaths(text: string): string[] {
  const paths: string[] = [];
  let current = '';
  let i = 0;

  while (i < text.length) {
    const char = text[i];

    if (char === '\\' && i + 1 < text.length && text[i + 1] === ' ') {
      current += '\\ ';
      i += 2;
    } else if (char === ' ') {
      if (current.trim()) {
        paths.push(current.trim());
      }
      current = '';
      i++;
    } else {
      current += char;
      i++;
    }
  }

  if (current.trim()) {
    paths.push(current.trim());
  }

  return paths;
}

export function parsePastedPaths(
  text: string,
  isValidPath: (path: string) => boolean,
): string | null {
  if (PATH_PREFIX_PATTERN.test(text) && isValidPath(text)) {
    return `@${escapePath(text)} `;
  }

  const segments = splitEscapedPaths(text);
  if (segments.length === 0) {
    return null;
  }

  let anyValidPath = false;
  const processedPaths = segments.map((segment) => {
    if (!PATH_PREFIX_PATTERN.test(segment)) {
      return segment;
    }
    const unescaped = unescapePath(segment);
    if (isValidPath(unescaped)) {
      anyValidPath = true;
      return `@${segment}`;
    }
    return segment;
  });

  return anyValidPath ? processedPaths.join(' ') + ' ' : null;
}
