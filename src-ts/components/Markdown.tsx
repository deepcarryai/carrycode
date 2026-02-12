import React, { useMemo } from 'react';
import { Text } from 'ink';
import { marked } from 'marked';
import TerminalRenderer from 'marked-terminal';
import chalk from 'chalk';
// @ts-ignore
import Table from 'cli-table3';
// @ts-ignore
import wrapAnsi from 'wrap-ansi';

interface MarkdownProps {
  children: string;
  width?: number;
  [key: string]: any;
}

const CELL_SPLIT = '§§CELL§§';
const ROW_SPLIT = '§§ROW§§';

export const Markdown: React.FC<MarkdownProps> = ({ children, width, ...options }) => {
  const content = useMemo(() => {
    const renderer = new TerminalRenderer({
      ...options,
      width: width,
      reflowText: false, // Disable built-in reflow to handle CJK manually
    });
    const originalCode = renderer.code.bind(renderer);

    const getSafeWidth = () => {
      if (!width || width <= 0) return undefined;
      return Math.max(10, width - 2);
    };

    const wrapByLine = (text: string, maxWidth: number) => {
      return text
        .split('\n')
        .map((line) => wrapAnsi(line, maxWidth, { hard: true, trim: true }))
        .join('\n');
    };

    const formatHangingIndent = (prefix: string, text: string, maxWidth: number) => {
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

    renderer.paragraph = function (this: any, text: string | any) {
      let content = text;
      if (typeof text === 'object') {
        if (text.tokens) {
           content = this.parser.parseInline(text.tokens);
        } else if (text.text) {
           content = text.text;
        } else {
         content = '';
        }
      }
      const safeWidth = getSafeWidth();
      if (!safeWidth) return String(content) + '\n\n';
      return wrapByLine(String(content), safeWidth) + '\n\n';
    };

    const LIST_ITEM_SPLIT = '§§ITEM§§';

    renderer.listitem = function (this: any, text: string | any) {
      let content = text;
      if (typeof text === 'object') {
        if (text.tokens) {
          content = this.parser.parseInline(text.tokens);
        } else if (text.text) {
          content = text.text;
        } else {
          content = '';
        }
      }
      return String(content) + LIST_ITEM_SPLIT;
    };

    renderer.list = function (this: any, body: string | any, ordered: boolean, start: number) {
      const safeWidth = getSafeWidth();
      if (typeof body === 'object' && body !== null && body.type === 'list' && Array.isArray(body.items)) {
        const token = body;
        const tokenOrdered = Boolean(token.ordered);
        const baseIndex = tokenOrdered ? (Number(token.start) || 1) : 1;

        const renderedItems = token.items.map((item: any, index: number) => {
          const prefix = tokenOrdered ? `${baseIndex + index}. ` : '- ';

          let itemContent = '';
          if (item?.tokens) {
            const hasBlockTokens = Array.isArray(item.tokens) && item.tokens.some((t: any) => {
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
      const items = rawBody.split(LIST_ITEM_SPLIT).map((s) => s.trimEnd()).filter((s) => s.length > 0);
      const baseIndex = Number.isFinite(start) && start > 0 ? start : 1;

      const renderedItems = items.map((item, index) => {
        const prefix = ordered ? `${baseIndex + index}. ` : '- ';
        if (!safeWidth) return prefix + item;
        return formatHangingIndent(prefix, item.trim(), safeWidth);
      });

      return renderedItems.join('\n') + '\n';
    };

    renderer.code = (code, language, escaped) => {
      // Get the highlighted code from the original renderer
      const highlighted = originalCode(code, language, escaped);
      
      // Split into lines
      const lines = highlighted.split('\n');
      
      // Calculate padding for line numbers
      const maxDigits = String(lines.length).length;
      
      // Add line numbers
      return lines.map((line, index) => {
        // Skip empty lines at the end which marked-terminal might add
        if (index === lines.length - 1 && line.trim() === '') return line;
        
        const lineNumber = String(index + 1).padStart(maxDigits, ' ');
        return chalk.gray(`${lineNumber} │ `) + line;
      }).join('\n');
    };

    renderer.table = function (this: any, header: string | any, body?: string) {
      try {
        let headRows: string[][] = [];
        let bodyRows: string[][] = [];

        if (typeof header === 'object' && header !== null) {
          // Token mode (marked >= 5.0.0 often passes tokens)
          const token = header;
          const parseCell = (cell: any) => {
            if (cell.tokens) {
              return this.parser.parseInline(cell.tokens);
            }
            return cell.text || '';
          };

          if (token.header) {
            headRows.push(token.header.map((cell: any) => parseCell(cell)));
          }
          if (token.rows) {
            bodyRows = token.rows.map((row: any) => row.map((cell: any) => parseCell(cell)));
          }
        } else {
          // String mode (legacy or simple string arguments)
          const parseRow = (rowStr: string) => {
            if (!rowStr) return [];
            return rowStr.split(CELL_SPLIT).slice(0, -1); // remove last empty split
          };

          headRows = header.split(ROW_SPLIT).filter((r: string) => r).map(parseRow);
          bodyRows = (body || '').split(ROW_SPLIT).filter((r: string) => r).map(parseRow);
        }

        if (headRows.length === 0 && bodyRows.length === 0) return '';

        const allRows = [...headRows, ...bodyRows];
        const colCount = allRows[0]?.length || 0;
        
        if (colCount === 0) return '';

        // Calculate column widths
        // Default to 80 if width is not provided or invalid
        const availableWidth = (width && width > 0) ? width : 80;
        
        // cli-table3 default style: 1 char border left, 1 char border right/mid.
        // Total table width = Sum(colWidths) + (colCount + 1)
        const totalBorderWidth = colCount + 1;
        const usableWidthForCols = Math.max(10, availableWidth - totalBorderWidth);
        const colWidthForTable = Math.floor(usableWidthForCols / colCount);
        
        // Default padding is 1 left + 1 right = 2
        const padding = 2;
        const contentWidth = Math.max(1, colWidthForTable - padding);

        const wrapCell = (cell: string) => {
          return wrapByLine(cell, contentWidth);
        };

        const wrappedHeadRows = headRows.map(row => row.map(wrapCell));
        const wrappedBodyRows = bodyRows.map(row => row.map(wrapCell));

        const table = new Table({
          head: wrappedHeadRows[0],
          colWidths: Array(colCount).fill(colWidthForTable),
          wordWrap: false, // We handle wrapping manually
          wrapOnWordBoundary: false,
          style: {
            head: ['cyan'],
            border: ['grey']
          }
        });

        // Add remaining header rows (unlikely in markdown)
        for (let i = 1; i < wrappedHeadRows.length; i++) {
          table.push(wrappedHeadRows[i]);
        }

        wrappedBodyRows.forEach(row => {
          if (row.length === colCount) {
             table.push(row);
          } else {
             // Handle mismatch cols?
             // Just push what we have, table might handle or we pad
             table.push(row);
          }
        });

        return table.toString() + '\n';
      } catch (e) {
        // Fallback in case of error
        // If header was an object, we can't just return it.
        if (typeof header === 'object') return '';
        return header + '\n' + (body || '');
      }
    };

    renderer.tablerow = function (content: string | any) {
      if (typeof content === 'object') {
        // Should not happen usually if we handled table() correctly, 
        // but marked might call this in some cases.
        return (content.text || '') + ROW_SPLIT;
      }
      return content + ROW_SPLIT;
    };

    renderer.tablecell = function (this: any, content: string | any, _flags: any) {
      if (typeof content === 'object') {
        if (content.tokens) {
           return this.parser.parseInline(content.tokens) + CELL_SPLIT;
        }
        return (content.text || '') + CELL_SPLIT;
      }
      return content + CELL_SPLIT;
    };

    marked.setOptions({
        // @ts-ignore
        renderer: renderer
    });
    const rendered = marked(children) as string;
    return rendered.trim();
  }, [children, options, width]);

  return <Text>{content}</Text>;
};

export default Markdown;
