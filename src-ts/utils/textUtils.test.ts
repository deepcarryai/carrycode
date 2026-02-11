import { getCachedStringWidth, padLeftToWidth, padRightToWidth, truncateToWidth } from './textUtils.js';

function expectEqual(actual: unknown, expected: unknown, name: string) {
  if (actual !== expected) {
    throw new Error(`${name}: expected ${String(expected)}, got ${String(actual)}`);
  }
}

function expectTrue(value: boolean, name: string) {
  if (!value) {
    throw new Error(`${name}: expected true`);
  }
}

(() => {
  expectEqual(getCachedStringWidth('abc'), 3, 'width ascii');
  expectEqual(getCachedStringWidth('你好'), 4, 'width cjk');

  const left = padLeftToWidth('下一步', 13);
  expectEqual(getCachedStringWidth(left), 13, 'padLeft width');

  const right = padRightToWidth('下一步', 13);
  expectEqual(getCachedStringWidth(right), 13, 'padRight width');

  const truncated = truncateToWidth('Hello 你好 World', 10);
  expectTrue(getCachedStringWidth(truncated) <= 10, 'truncate width <= limit');
})();

