import { describe, expect, it } from 'vitest';
import { frameworkModulesReachable, type BundleChunk } from './framework-free-entries.ts';

const REACT = '/repo/node_modules/.pnpm/react@19.3.1/node_modules/react/index.js';
const MANTINE =
  '/repo/node_modules/.pnpm/@mantine+core@9.7.5/node_modules/@mantine/core/esm/index.mjs';

function chunk(overrides: Partial<BundleChunk>): BundleChunk {
  return {
    type: 'chunk',
    isEntry: false,
    imports: [],
    dynamicImports: [],
    moduleIds: [],
    ...overrides,
  };
}

describe('frameworkModulesReachable()', () => {
  it('reports a framework module reached through a shared chunk', () => {
    const bundle = {
      'index.js': chunk({ name: 'index', isEntry: true, imports: ['shared.js'] }),
      'shared.js': chunk({ name: 'shared', moduleIds: ['/repo/src/dom.ts', REACT] }),
    };

    expect(frameworkModulesReachable(bundle, ['index', 'tabbar'])).toEqual([`index → ${REACT}`]);
  });

  it('reports a framework module reached through a dynamic import', () => {
    const bundle = {
      'tabbar.js': chunk({ name: 'tabbar', isEntry: true, dynamicImports: ['lazy.js'] }),
      'lazy.js': chunk({ name: 'lazy', moduleIds: [MANTINE] }),
    };

    expect(frameworkModulesReachable(bundle, ['index', 'tabbar'])).toEqual([`tabbar → ${MANTINE}`]);
  });

  it('ignores an entry that is not listed', () => {
    const bundle = {
      'board.js': chunk({ name: 'board', isEntry: true, moduleIds: [REACT] }),
    };

    expect(frameworkModulesReachable(bundle, ['index', 'tabbar'])).toEqual([]);
  });

  it('reports nothing for entries made of source modules only', () => {
    const bundle = {
      'index.js': chunk({ name: 'index', isEntry: true, moduleIds: ['/repo/src/main.ts'] }),
    };

    expect(frameworkModulesReachable(bundle, ['index'])).toEqual([]);
  });

  it('terminates on chunks that import each other', () => {
    const bundle = {
      'index.js': chunk({ name: 'index', isEntry: true, imports: ['a.js'] }),
      'a.js': chunk({ name: 'a', imports: ['b.js'] }),
      'b.js': chunk({ name: 'b', imports: ['a.js'], moduleIds: [REACT] }),
    };

    expect(frameworkModulesReachable(bundle, ['index'])).toEqual([`index → ${REACT}`]);
  });
});
