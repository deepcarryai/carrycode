import { marked } from 'marked';
import TerminalRenderer from 'marked-terminal';
import chalk from 'chalk';
import Table from 'cli-table3';
import wrapAnsi from 'wrap-ansi';

const CELL_SPLIT = '§§CELL§§';
const ROW_SPLIT = '§§ROW§§';
const LIST_ITEM_SPLIT = '§§ITEM§§';

const stripAnsi = (text) => text.replace(/\x1b\[[0-9;]*m/g, '');

const renderMarkdownToTerminalText = (markdown, width) => {
  const renderer = new TerminalRenderer({ width, reflowText: false });
  const originalCode = renderer.code.bind(renderer);

  const getSafeWidth = () => {
    if (!width || width <= 0) return undefined;
    return Math.max(10, width - 2);
  };

  const wrapByLine = (text, maxWidth) =>
    String(text)
      .split('\n')
      .map((line) => wrapAnsi(line, maxWidth, { hard: true, trim: true }))
      .join('\n');

  const formatHangingIndent = (prefix, text, maxWidth) => {
    const prefixWidth = prefix.length;
    const bodyWidth = Math.max(10, maxWidth - prefixWidth);
    const wrapped = wrapByLine(text, bodyWidth);
    const lines = wrapped.split('\n');
    const continuationIndent = ' '.repeat(prefixWidth);
    return lines
      .map((line, index) => {
        if (index === 0) return prefix + line;
        if (!line) return line;
        return continuationIndent + line;
      })
      .join('\n');
  };

  renderer.paragraph = function (text) {
    let content = text;
    if (typeof text === 'object') {
      if (text.tokens) content = this.parser.parseInline(text.tokens);
      else if (text.text) content = text.text;
      else content = '';
    }

    const safeWidth = getSafeWidth();
    if (!safeWidth) return String(content) + '\n\n';
    return wrapByLine(String(content), safeWidth) + '\n\n';
  };

  renderer.listitem = function (text) {
    let content = text;
    if (typeof text === 'object') {
      if (text.tokens) content = this.parser.parseInline(text.tokens);
      else if (text.text) content = text.text;
      else content = '';
    }
    return String(content) + LIST_ITEM_SPLIT;
  };

  renderer.list = function (body, ordered, start) {
    const safeWidth = getSafeWidth();
    if (typeof body === 'object' && body !== null && body.type === 'list' && Array.isArray(body.items)) {
      const token = body;
      const tokenOrdered = Boolean(token.ordered);
      const baseIndex = tokenOrdered ? (Number(token.start) || 1) : 1;

      const renderedItems = token.items.map((item, index) => {
        const prefix = tokenOrdered ? `${baseIndex + index}. ` : '- ';

        let itemContent = '';
        if (item?.tokens) {
          const hasBlockTokens = Array.isArray(item.tokens) && item.tokens.some((t) => {
            const type = t?.type;
            return type && type !== 'text';
          });
          itemContent = hasBlockTokens ? this.parser.parse(item.tokens).trimEnd() : this.parser.parseInline(item.tokens);
        } else if (typeof item?.text === 'string') {
          itemContent = item.text;
        } else {
          itemContent = String(item ?? '');
        }

        if (!safeWidth) return prefix + itemContent;
        return formatHangingIndent(prefix, itemContent.trim(), safeWidth);
      });

      return renderedItems.join('\n') + '\n';
    }
    const rawBody = typeof body === 'string' ? body : String(body ?? '');
    const items = rawBody
      .split(LIST_ITEM_SPLIT)
      .map((s) => s.trimEnd())
      .filter((s) => s.length > 0);
    const baseIndex = Number.isFinite(start) && start > 0 ? start : 1;

    const renderedItems = items.map((item, index) => {
      const prefix = ordered ? `${baseIndex + index}. ` : '- ';
      if (!safeWidth) return prefix + item;
      return formatHangingIndent(prefix, item.trim(), safeWidth);
    });

    return renderedItems.join('\n') + '\n';
  };

  renderer.code = (code, language, escaped) => {
    const highlighted = originalCode(code, language, escaped);
    const lines = highlighted.split('\n');
    const maxDigits = String(lines.length).length;

    return lines
      .map((line, index) => {
        if (index === lines.length - 1 && line.trim() === '') return line;
        const lineNumber = String(index + 1).padStart(maxDigits, ' ');
        return chalk.gray(`${lineNumber} │ `) + line;
      })
      .join('\n');
  };

  renderer.table = function (header, body) {
    let headRows = [];
    let bodyRows = [];

    if (typeof header === 'object' && header !== null) {
      const token = header;
      const parseCell = (cell) => {
        if (cell.tokens) return this.parser.parseInline(cell.tokens);
        return cell.text || '';
      };
      if (token.header) headRows.push(token.header.map((cell) => parseCell(cell)));
      if (token.rows) bodyRows = token.rows.map((row) => row.map((cell) => parseCell(cell)));
    } else {
      const parseRow = (rowStr) => {
        if (!rowStr) return [];
        return rowStr.split(CELL_SPLIT).slice(0, -1);
      };
      headRows = String(header ?? '').split(ROW_SPLIT).filter((r) => r).map(parseRow);
      bodyRows = String(body ?? '').split(ROW_SPLIT).filter((r) => r).map(parseRow);
    }

    if (headRows.length === 0 && bodyRows.length === 0) return '';

    const allRows = [...headRows, ...bodyRows];
    const colCount = allRows[0]?.length || 0;
    if (colCount === 0) return '';

    const availableWidth = width && width > 0 ? width : 80;
    const totalBorderWidth = colCount + 1;
    const usableWidthForCols = Math.max(10, availableWidth - totalBorderWidth);
    const colWidthForTable = Math.floor(usableWidthForCols / colCount);
    const padding = 2;
    const contentWidth = Math.max(1, colWidthForTable - padding);
    const wrapCell = (cell) => wrapByLine(cell, contentWidth);

    const wrappedHeadRows = headRows.map((row) => row.map(wrapCell));
    const wrappedBodyRows = bodyRows.map((row) => row.map(wrapCell));

    const table = new Table({
      head: wrappedHeadRows[0],
      colWidths: Array(colCount).fill(colWidthForTable),
      wordWrap: false,
      wrapOnWordBoundary: false,
      style: { head: ['cyan'], border: ['grey'] },
    });

    for (let i = 1; i < wrappedHeadRows.length; i++) table.push(wrappedHeadRows[i]);
    wrappedBodyRows.forEach((row) => table.push(row));
    return table.toString() + '\n';
  };

  renderer.tablerow = function (content) {
    if (typeof content === 'object') return (content.text || '') + ROW_SPLIT;
    return content + ROW_SPLIT;
  };

  renderer.tablecell = function (content) {
    if (typeof content === 'object') {
      if (content.tokens) return this.parser.parseInline(content.tokens) + CELL_SPLIT;
      return (content.text || '') + CELL_SPLIT;
    }
    return content + CELL_SPLIT;
  };

  marked.setOptions({ renderer });
  return String(marked(markdown)).trimEnd();
};

const validateNoOverIndent = (rendered) => {
  const lines = rendered.split('\n');
  for (const line of lines) {
    const plain = stripAnsi(line);
    const leadingSpaces = (plain.match(/^ */) ?? [''])[0].length;
    if (leadingSpaces >= 8) {
      throw new Error(`发现疑似过度缩进（>=8 空格）：${JSON.stringify(plain)}`);
    }
  }
};

const validateHangingIndentForSimpleLists = (rendered) => {
  const lines = rendered.split('\n').map(stripAnsi);
  const listItemPrefixRegex = /^(- |\d+\. )/;
  let expectedContinuationIndent = null;

  for (const line of lines) {
    if (!line.trim()) {
      expectedContinuationIndent = null;
      continue;
    }

    const match = line.match(listItemPrefixRegex);
    if (match) {
      expectedContinuationIndent = match[1].length;
      continue;
    }

    const leadingSpaces = (line.match(/^ */) ?? [''])[0].length;
    if (expectedContinuationIndent != null && leadingSpaces === 0) {
      expectedContinuationIndent = null;
    }
    if (expectedContinuationIndent != null) {
      if (leadingSpaces !== expectedContinuationIndent) {
        throw new Error(
          `列表续行缩进异常：期望 ${expectedContinuationIndent}，实际 ${leadingSpaces}，行：${JSON.stringify(line)}`
        );
      }
    } else if (leadingSpaces !== 0) {
      throw new Error(`普通段落续行缩进异常：${JSON.stringify(line)}`);
    }
  }
};

const md = [
  '这是一个很长的普通段落，用来触发自动换行并检查第二行不会被莫名其妙地大幅缩进。为了更稳定地复现，我们让这段文本足够长，包含中文与 English words 混排。',
  '',
  '- 这是一个很长的无序列表项，用来触发换行并检查续行缩进仅为两个空格（等于 \"- \" 的宽度），而不是出现更大的缩进。',
  '- 第二个列表项同样足够长，确保会发生换行，从而覆盖更多行的续行对齐逻辑。',
  '',
  '1. 这是一个很长的有序列表项，用来触发换行并检查续行缩进仅等于序号前缀宽度。',
  '2. 第二个有序列表项同样足够长，确保会发生换行。',
  '',
  '| 表头A | 表头B |',
  '| --- | --- |',
  '| 这里是一段很长的表格单元格内容，用来触发换行并确保不会出现过度缩进 | 这里也是很长的单元格内容，包含中文与 English words 混排 |',
].join('\n');

const width = Number.parseInt(process.env.MD_WIDTH ?? '40', 10);
const rendered = renderMarkdownToTerminalText(md, width);

try {
  validateNoOverIndent(rendered);
  validateHangingIndentForSimpleLists(rendered);
  process.stdout.write(rendered + '\n\nOK\n');
} catch (err) {
  process.stderr.write(rendered + '\n\nFAIL\n');
  process.stderr.write(String(err?.stack ?? err) + '\n');
  process.exitCode = 1;
}
