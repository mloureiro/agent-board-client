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
  location: { origin: string; href: string; pathname: string; search: string };
  addEventListener: (
    type: string,
    listener: (event: FakeKeyEvent) => void,
    capture?: boolean,
  ) => void;
  EventTarget: typeof EventTarget;
  Notification?: {
    permission: string;
    requestPermission: (callback?: (permission: string) => void) => Promise<string>;
    new (title: string): EventTarget & { close: () => void };
  };
}

interface FakeKeyEvent {
  key?: string;
  ctrlKey?: boolean;
  metaKey?: boolean;
  shiftKey?: boolean;
  preventDefault?: () => void;
}

function setup(role = 'home') {
  const original = { pushState: vi.fn(), replaceState: vi.fn() };
  const assigned: string[] = [];
  const listeners: Record<string, ((event: FakeKeyEvent) => void)[]> = {};
  const win: FakeWindow = {
    URL,
    EventTarget,
    addEventListener: (type, listener) => {
      (listeners[type] ??= []).push(listener);
    },
    history: { ...original },
    location: {
      origin: ORIGIN,
      pathname: '/t/current',
      search: '?transcript=1',
      get href() {
        return `${ORIGIN}/`;
      },
      set href(value: string) {
        assigned.push(value);
      },
    },
  };
  (runInNewContext(script) as Install)(role, win);
  const fire = (type: string, event: FakeKeyEvent = {}) => {
    listeners[type]?.forEach((listener) => {
      listener(event);
    });
  };
  return { win, original, assigned, fire };
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

  it('leaves Notification alone', () => {
    const { win } = setup();

    expect(win.Notification).toBeUndefined();
  });
});

describe('tab-routing tab role', () => {
  describe.each(['pushState', 'replaceState'] as const)('%s', (method) => {
    it.each(['/', '/?x=1'])('swallows the home target %s and reports it', (path) => {
      const { win, original, assigned } = setup('tab');

      win.history[method](null, '', path);

      expect(assigned).toEqual([`agent-board-client:route?path=${encodeURIComponent(path)}`]);
      expect(original[method]).not.toHaveBeenCalled();
    });

    it('calls the original for a page, then reports it', () => {
      const { win, original, assigned } = setup('tab');
      original[method].mockImplementation(() => {
        expect(assigned).toEqual([]);
      });

      win.history[method](null, '', '/t/other?transcript=1');

      expect(original[method]).toHaveBeenCalledTimes(1);
      expect(assigned).toEqual([
        `agent-board-client:route?path=${encodeURIComponent('/t/other?transcript=1')}`,
      ]);
    });

    it('does not report a call with no target or a cross-origin one', () => {
      const { win, original, assigned } = setup('tab');

      win.history[method](null, '');
      win.history[method](null, '', 'https://other.example.com/');

      expect(original[method]).toHaveBeenCalledTimes(2);
      expect(assigned).toEqual([]);
    });
  });

  it('reports the new location on popstate', () => {
    const { assigned, fire } = setup('tab');

    fire('popstate');

    expect(assigned).toEqual([
      `agent-board-client:route?path=${encodeURIComponent('/t/current?transcript=1')}`,
    ]);
  });

  it('replaces Notification with one that is granted and shows nothing', async () => {
    const { win } = setup('tab');
    const Stand = win.Notification;
    const callback = vi.fn();

    expect(Stand?.permission).toBe('granted');
    await expect(Stand?.requestPermission(callback)).resolves.toBe('granted');
    expect(callback).toHaveBeenCalledWith('granted');
    expect(() => {
      const shown = new (Stand as NonNullable<typeof Stand>)('title');
      shown.close();
    }).not.toThrow();
  });
});

describe.each(['home', 'tab'])('tab-routing %s role keyboard', (role) => {
  it.each([
    ['Ctrl+Tab', { ctrlKey: true }, 'next'],
    ['Ctrl+Shift+Tab', { ctrlKey: true, shiftKey: true }, 'previous'],
  ])('turns %s into a cycle signal', (_name, modifiers, dir) => {
    const { assigned, fire } = setup(role);
    const preventDefault = vi.fn();

    fire('keydown', { key: 'Tab', preventDefault, ...modifiers });

    expect(assigned).toEqual([`agent-board-client:cycle?dir=${dir}`]);
    expect(preventDefault).toHaveBeenCalledTimes(1);
  });

  it.each([
    ['a plain Tab', { key: 'Tab' }],
    ['Cmd+Ctrl+Tab', { key: 'Tab', ctrlKey: true, metaKey: true }],
    ['Ctrl+W', { key: 'w', ctrlKey: true }],
  ])('leaves %s to the page', (_name, event) => {
    const { assigned, fire } = setup(role);
    const preventDefault = vi.fn();

    fire('keydown', { preventDefault, ...event });

    expect(assigned).toEqual([]);
    expect(preventDefault).not.toHaveBeenCalled();
  });
});
