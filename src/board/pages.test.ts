import { describe, expect, it } from 'vitest';
import pageCases from '../../fixtures/page-cases.json';
import nativePages from '../../fixtures/native-pages.json';
import tabKindCases from '../../fixtures/tab-kind-cases.json';
import { isNativePath, NATIVE_PAGES, pageKindOf, parseNativePages } from './pages.ts';

describe('pageKindOf()', () => {
  it.each(pageCases)('classifies $path', ({ path, route, page }) => {
    const expected = route === 'home' ? 'home' : route === 'page' ? page?.split(':')[0] : null;

    expect(pageKindOf(path)).toBe(expected);
  });
});

describe('isNativePath()', () => {
  const rows = tabKindCases.filter((row) => !row.boardOnly);

  it.each(rows)('gives $kind for $path with $pages', ({ pages, path, kind }) => {
    expect(isNativePath(path, parseNativePages(pages))).toBe(kind === 'native');
  });
});

describe('parseNativePages()', () => {
  it('refuses a value that is not an array', () => {
    expect(() => parseNativePages({})).toThrow(TypeError);
  });

  it('refuses an unknown page kind', () => {
    expect(() => parseNativePages(['home', 'nope'])).toThrow(TypeError);
  });

  it('reads the bundled table', () => {
    expect(NATIVE_PAGES).toEqual(parseNativePages(nativePages));
  });
});
