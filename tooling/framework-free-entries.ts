import type { Plugin } from 'vite';

export const FRAMEWORK_MODULE =
  /[\\/]node_modules[\\/](react|react-dom|scheduler|react-router|react-router-dom|@mantine|@tanstack)[\\/]/;

export interface BundleChunk {
  type: string;
  name?: string;
  isEntry?: boolean;
  imports?: readonly string[];
  dynamicImports?: readonly string[];
  moduleIds?: readonly string[];
}

export function frameworkModulesReachable(
  bundle: Readonly<Record<string, BundleChunk>>,
  entries: readonly string[],
): string[] {
  const found: string[] = [];

  for (const root of Object.values(bundle)) {
    if (root.type !== 'chunk' || !root.isEntry || !root.name || !entries.includes(root.name)) {
      continue;
    }

    const seen = new Set<string>();
    const pending = [root];
    for (let chunk = pending.pop(); chunk; chunk = pending.pop()) {
      for (const id of chunk.moduleIds ?? []) {
        if (FRAMEWORK_MODULE.test(id)) {
          found.push(`${root.name} → ${id}`);
        }
      }
      for (const file of [...(chunk.imports ?? []), ...(chunk.dynamicImports ?? [])]) {
        const next = bundle[file];
        if (next?.type === 'chunk' && !seen.has(file)) {
          seen.add(file);
          pending.push(next);
        }
      }
    }
  }

  return found;
}

export function frameworkFreeEntries(entries: readonly string[]): Plugin {
  return {
    name: 'framework-free-entries',
    apply: 'build',
    generateBundle(_options, bundle) {
      const found = frameworkModulesReachable(bundle, entries);
      if (found.length > 0) {
        this.error(
          `These entries must stay free of the native app's framework code, which only board.html may load:\n${found.join('\n')}`,
        );
      }
    },
  };
}
