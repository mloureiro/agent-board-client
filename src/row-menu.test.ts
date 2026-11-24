import { describe, expect, it } from 'vitest';
import { nextMenuIndex } from './row-menu';

describe('nextMenuIndex', () => {
  it.each([
    { key: 'ArrowDown', index: 0, expected: 1 },
    { key: 'ArrowDown', index: 2, expected: 0 },
    { key: 'ArrowUp', index: 1, expected: 0 },
    { key: 'ArrowUp', index: 0, expected: 2 },
    { key: 'Home', index: 2, expected: 0 },
    { key: 'End', index: 0, expected: 2 },
    { key: 'a', index: 1, expected: 1 },
  ])('maps $key at item $index to item $expected', ({ key, index, expected }) => {
    expect(nextMenuIndex(key, index, 3)).toBe(expected);
  });
});
