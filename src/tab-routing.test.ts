import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';
import { describe, expect, it, vi } from 'vitest';
import cases from '../fixtures/page-cases.json' with { type: 'json' };

const ORIGIN = 'https://board.example.com';
const script = readFileSync(new URL('../src-tauri/scripts/tab-routing.js', import.meta.url), 'utf8')
  .trim()
  .replace(/;$/, '');

type Install = (role: string, win: FakeWindow) => void;

interface FakeWindow {
  URL: typeof URL;
  history: {
    pushState: (...args: unknown[]) => unknown;
    replaceState: (...args: unknown[]) => unknown;
  };
  location: { origin: string; href: string };
}

function setup(role = 'home') {
  const original = { pushState: vi.fn(), replaceState: vi.fn() };
  const assigned: string[] = [];
  const win: FakeWindow = {
    URL,
    history: { ...original },
    location: {
      origin: ORIGIN,
      get href() {
        return `${ORIGIN}/`;
      },
      set href(value: string) {
        assigned.push(value);
      },
    },
  };
  (runInNewContext(script) as Install)(role, win);
  return { win, original, assigned };
}

describe('tab-routing home role', () => {
  describe.each(['pushState', 'replaceState'] as const)('%s', (method) => {
    it.each(cases)('$path', ({ path, route }) => {
      const { win, original, assigned } = setup();

      win.history[method](null, '', path);

      if (route === 'page') {
        const target = new URL(path, ORIGIN);
        expect(assigned).toEqual([
          `agent-board-client:open?path=${encodeURIComponent(target.pathname + target.search)}`,
        ]);
        expect(original[method]).not.toHaveBeenCalled();
      } else {
        expect(assigned).toEqual([]);
        expect(original[method]).toHaveBeenCalledTimes(1);
      }
    });

    it('passes a cross-origin target to the original', () => {
      const { win, original, assigned } = setup();

      win.history[method](null, '', 'https://other.example.com/backlog');

      expect(assigned).toEqual([]);
      expect(original[method]).toHaveBeenCalledTimes(1);
    });

    it('passes a call with no target to the original', () => {
      const { win, original, assigned } = setup();

      win.history[method](null, '');

      expect(assigned).toEqual([]);
      expect(original[method]).toHaveBeenCalledTimes(1);
    });

    it('falls back to the original when sending the signal throws', () => {
      const { win, original } = setup();
      Object.defineProperty(win.location, 'href', {
        get: () => `${ORIGIN}/`,
        set: () => {
          throw new Error('blocked');
        },
      });

      win.history[method](null, '', '/backlog');

      expect(original[method]).toHaveBeenCalledTimes(1);
    });
  });

  it('leaves history untouched in the tab role', () => {
    const { win, original } = setup('tab');

    win.history.pushState(null, '', '/backlog');

    expect(original.pushState).toHaveBeenCalledTimes(1);
  });
});
