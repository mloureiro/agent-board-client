import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import cases from '../fixtures/page-cases.json' with { type: 'json' };

const ORIGIN = 'https://board.example.com';
const script = readFileSync(new URL('../src-tauri/scripts/tab-routing.js', import.meta.url), 'utf8')
  .trim()
  .replace(/;$/, '');

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

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
  setTimeout: (callback: () => void, delay: number) => unknown;
  fetch?: (...args: unknown[]) => Promise<unknown>;
  Date: { now: () => number };
  ServiceWorkerRegistration?: { prototype: { showNotification?: unknown } };
  Notification?: {
    permission: string;
    requestPermission: (callback?: (permission: string) => void) => Promise<string>;
    new (title: string, options?: object): EventTarget & { close: () => void };
  };
}

interface FakeKeyEvent {
  key?: string;
  ctrlKey?: boolean;
  metaKey?: boolean;
  shiftKey?: boolean;
  preventDefault?: () => void;
}

function setup(
  role = 'home',
  withRegistration = false,
  fetch?: (...args: unknown[]) => Promise<unknown>,
) {
  const clock = { now: 0 };
  const original = { pushState: vi.fn(), replaceState: vi.fn() };
  const assigned: string[] = [];
  const listeners: Record<string, ((event: FakeKeyEvent) => void)[]> = {};
  const win: FakeWindow = {
    URL,
    EventTarget,
    Date: { now: () => clock.now },
    setTimeout: (callback, delay) => setTimeout(callback, delay),
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
  if (fetch) {
    win.fetch = fetch;
  }
  if (withRegistration) {
    win.ServiceWorkerRegistration = { prototype: {} };
  }
  (runInNewContext(script) as Install)(role, win);
  const fire = (type: string, event: FakeKeyEvent = {}) => {
    listeners[type]?.forEach((listener) => {
      listener(event);
    });
  };
  return { win, original, assigned, fire, clock, flush: () => vi.runAllTimers() };
}

describe('tab-routing home role', () => {
  describe.each(['pushState', 'replaceState'] as const)('%s', (method) => {
    it.each(cases)('$path', ({ path, route }) => {
      const { win, original, assigned, flush } = setup();

      win.history[method](null, '', path);
      flush();

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
      const { win, original, assigned, flush } = setup();

      win.history[method](null, '', 'https://other.example.com/backlog');
      flush();

      expect(assigned).toEqual([]);
      expect(original[method]).toHaveBeenCalledTimes(1);
    });

    it('passes a call with no target to the original', () => {
      const { win, original, assigned, flush } = setup();

      win.history[method](null, '');
      flush();

      expect(assigned).toEqual([]);
      expect(original[method]).toHaveBeenCalledTimes(1);
    });
  });
});

describe('tab-routing home role notifications', () => {
  type Show = (title: string, options?: object) => Promise<unknown>;

  function notifyQuery(assigned: string[]) {
    expect(assigned).toHaveLength(1);
    const url = new URL(assigned[0] as string);
    expect(url.protocol).toBe('agent-board-client:');
    expect(url.pathname).toBe('notify');
    return url.searchParams;
  }

  function show(via: 'constructor' | 'worker', title: string, options: object) {
    const context = setup('home', true);
    if (via === 'constructor') {
      new (context.win.Notification as NonNullable<typeof context.win.Notification>)(
        title,
        options,
      );
    } else {
      const registration = context.win.ServiceWorkerRegistration as {
        prototype: { showNotification: Show };
      };
      void registration.prototype.showNotification(title, options);
    }
    context.flush();
    return context.assigned;
  }

  it.each([
    [
      'constructor',
      { body: 'Fix the build', tag: 'task-1' },
      { title: 'Needs you', body: 'Fix the build', tag: 'task-1', path: null },
    ],
    [
      'worker',
      { body: 'Fix the build', tag: 'task-1', data: { path: '/t/k3J9' } },
      { title: 'Needs you', body: 'Fix the build', tag: 'task-1', path: '/t/k3J9' },
    ],
    ['constructor', {}, { title: 'Needs you', body: '', tag: null, path: null }],
  ] as const)('forwards a call through the %s', (via, options, expected) => {
    const query = notifyQuery(show(via, 'Needs you', options));

    expect({
      title: query.get('title'),
      body: query.get('body'),
      tag: query.get('tag'),
      path: query.get('path'),
    }).toEqual(expected);
  });

  it('cuts a long title to 200 code points ending in an ellipsis', () => {
    const query = notifyQuery(show('worker', '\u{1F600}'.repeat(300), {}));

    const title = Array.from(query.get('title') as string);
    expect(title).toHaveLength(200);
    expect(title.at(-1)).toBe('\u2026');
  });

  it('cuts a long body to 500 code points ending in an ellipsis', () => {
    const query = notifyQuery(show('worker', 'x', { body: 'b'.repeat(600) }));

    const body = Array.from(query.get('body') as string);
    expect(body).toHaveLength(500);
    expect(body.at(-1)).toBe('\u2026');
  });

  it('leaves out a tag over 200 code points', () => {
    const query = notifyQuery(show('worker', 'x', { tag: 't'.repeat(201) }));

    expect(query.has('tag')).toBe(false);
  });

  it('sends three notifications of one task as three signals', () => {
    const { win, assigned, flush } = setup('home', true);
    const registration = win.ServiceWorkerRegistration as {
      prototype: { showNotification: Show };
    };

    for (const title of ['one', 'two', 'three']) {
      void registration.prototype.showNotification(title, {});
    }
    flush();

    expect(assigned.map((href) => new URL(href).searchParams.get('title'))).toEqual([
      'one',
      'two',
      'three',
    ]);
  });

  it('reports permission granted and resolves a permission request', async () => {
    const { win } = setup('home');
    const callback = vi.fn();

    expect(win.Notification?.permission).toBe('granted');
    await expect(win.Notification?.requestPermission(callback)).resolves.toBe('granted');
    expect(callback).toHaveBeenCalledWith('granted');
  });
});

describe('tab-routing sentinel queue', () => {
  it('assigns every signal of one task in order, one per timer task', () => {
    const { win, assigned, fire } = setup();

    win.history.pushState(null, '', '/settings');
    fire('keydown', { key: 'Tab', ctrlKey: true, preventDefault: vi.fn() });

    expect(assigned).toEqual([]);
    vi.advanceTimersToNextTimer();
    expect(assigned).toEqual([`agent-board-client:open?path=${encodeURIComponent('/settings')}`]);
    vi.advanceTimersToNextTimer();
    expect(assigned).toEqual([
      `agent-board-client:open?path=${encodeURIComponent('/settings')}`,
      'agent-board-client:cycle?dir=next',
    ]);
  });

  it('keeps draining after an assignment throws', () => {
    const { win, assigned, fire, flush } = setup();
    Object.defineProperty(win.location, 'href', {
      get: () => `${ORIGIN}/`,
      set: (value: string) => {
        if (value.includes('open')) {
          throw new Error('blocked');
        }
        assigned.push(value);
      },
    });

    win.history.pushState(null, '', '/settings');
    fire('keydown', { key: 'Tab', ctrlKey: true, preventDefault: vi.fn() });

    expect(() => flush()).toThrow('blocked');
    flush();
    expect(assigned).toEqual(['agent-board-client:cycle?dir=next']);
  });
});

describe('tab-routing tab role', () => {
  describe.each(['pushState', 'replaceState'] as const)('%s', (method) => {
    it.each(['/', '/?x=1'])('swallows the home target %s and reports it', (path) => {
      const { win, original, assigned, flush } = setup('tab');

      win.history[method](null, '', path);
      flush();

      expect(assigned).toEqual([`agent-board-client:route?path=${encodeURIComponent(path)}`]);
      expect(original[method]).not.toHaveBeenCalled();
    });

    it('calls the original for a page, then reports it', () => {
      const { win, original, assigned, flush } = setup('tab');
      original[method].mockImplementation(() => {
        expect(assigned).toEqual([]);
      });

      win.history[method](null, '', '/t/other?transcript=1');
      flush();

      expect(original[method]).toHaveBeenCalledTimes(1);
      expect(assigned).toEqual([
        `agent-board-client:route?path=${encodeURIComponent('/t/other?transcript=1')}`,
      ]);
    });

    it('does not report a call with no target or a cross-origin one', () => {
      const { win, original, assigned, flush } = setup('tab');

      win.history[method](null, '');
      win.history[method](null, '', 'https://other.example.com/');
      flush();

      expect(original[method]).toHaveBeenCalledTimes(2);
      expect(assigned).toEqual([]);
    });
  });

  it('reports the new location on popstate', () => {
    const { assigned, fire, flush } = setup('tab');

    fire('popstate');
    flush();

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

  it('stands in for the service worker notification without sending anything', async () => {
    const { win, assigned, flush } = setup('tab', true);
    const registration = win.ServiceWorkerRegistration as {
      prototype: { showNotification: (title: string, options: object) => Promise<unknown> };
    };

    await expect(registration.prototype.showNotification('x', {})).resolves.toBeUndefined();
    flush();

    expect(assigned).toEqual([]);
  });

  it('installs without a ServiceWorkerRegistration', () => {
    expect(() => setup('tab', false)).not.toThrow();
  });
});

describe.each(['home', 'tab'])('tab-routing %s role keyboard', (role) => {
  it.each([
    ['Ctrl+Tab', { ctrlKey: true }, 'next'],
    ['Ctrl+Shift+Tab', { ctrlKey: true, shiftKey: true }, 'previous'],
  ])('turns %s into a cycle signal', (_name, modifiers, dir) => {
    const { assigned, fire, flush } = setup(role);
    const preventDefault = vi.fn();

    fire('keydown', { key: 'Tab', preventDefault, ...modifiers });
    flush();

    expect(assigned).toEqual([`agent-board-client:cycle?dir=${dir}`]);
    expect(preventDefault).toHaveBeenCalledTimes(1);
  });

  it.each([
    ['a plain Tab', { key: 'Tab' }],
    ['Cmd+Ctrl+Tab', { key: 'Tab', ctrlKey: true, metaKey: true }],
    ['Ctrl+W', { key: 'w', ctrlKey: true }],
  ])('leaves %s to the page', (_name, event) => {
    const { assigned, fire, flush } = setup(role);
    const preventDefault = vi.fn();

    fire('keydown', { preventDefault, ...event });
    flush();

    expect(assigned).toEqual([]);
    expect(preventDefault).not.toHaveBeenCalled();
  });
});

describe('tab-routing server health', () => {
  interface Reply {
    status?: number;
    redirected?: boolean;
    type?: string;
    contentType?: string;
  }

  const answer = ({
    status = 200,
    redirected = false,
    type = 'basic',
    contentType = 'application/json',
  }: Reply = {}) => ({
    status,
    redirected,
    type,
    headers: { get: (name: string) => (name === 'content-type' ? contentType : null) },
  });

  type Fetch = (input: unknown, init?: { redirect?: string }) => Promise<unknown>;

  function board(script: Fetch, role = 'home') {
    const calls: { input: unknown; init?: { redirect?: string } }[] = [];
    const context = setup(role, false, (input, init) => {
      calls.push({ input, init: init as { redirect?: string } | undefined });
      return script(input, init as { redirect?: string } | undefined);
    });
    const request = async (input: unknown) => {
      const promise = (context.win.fetch as Fetch)(input);
      await promise.catch(() => undefined);
      await vi.advanceTimersByTimeAsync(0);
      return promise;
    };
    const sent = () => context.assigned.filter((href) => href.includes(':health'));
    return { ...context, calls, request, sent };
  }

  const states = (hrefs: string[]) => hrefs.map((href) => new URL(href).searchParams.get('state'));

  describe('which requests are observed', () => {
    it.each([
      ['a relative trpc path', '/trpc/tasks.list?batch=1', true],
      ['an absolute same-origin trpc url', `${ORIGIN}/trpc/x`, true],
      ['a Request object', new Request(`${ORIGIN}/trpc/x`), true],
      ['the bare trpc path', '/trpc', true],
      ['another api path', '/api/x', false],
      ['a path that only starts with the word', '/trpcx', false],
      ['a cross-origin trpc url', 'https://other.example.com/trpc/x', false],
    ])('%s', async (_name, input, observed) => {
      const { request, sent } = board(() => Promise.resolve(answer()));

      await request(input);

      expect(states(sent())).toEqual(observed ? ['ok'] : []);
    });
  });

  describe('what a response means', () => {
    it.each([
      ['401', { status: 401 }, 'session'],
      ['a redirect', { redirected: true }, 'session'],
      ['an html page', { contentType: 'text/html; charset=utf-8' }, 'session'],
      ['200', { status: 200 }, 'ok'],
      ['404', { status: 404 }, 'ok'],
      ['500', { status: 500 }, 'ok'],
    ] as [string, Reply, string][])('%s', async (_name, reply, expected) => {
      const { request, sent } = board(() => Promise.resolve(answer(reply)));

      await request('/trpc/x');

      expect(states(sent())).toEqual([expected]);
    });

    it.each([502, 503, 504])('%i counts as a failure that turns into down', async (status) => {
      const { request, sent, clock } = board(() => Promise.resolve(answer({ status })));

      await request('/trpc/x');
      clock.now = 1000;
      await request('/trpc/x');
      expect(sent()).toEqual([]);
      clock.now = 8000;
      await request('/trpc/x');

      expect(states(sent())).toEqual(['down']);
    });
  });

  describe('the probe after a rejection', () => {
    it.each([
      ['rejects', () => Promise.reject(new TypeError('offline')), 'down'],
      [
        'opaqueredirect',
        () => Promise.resolve(answer({ status: 0, type: 'opaqueredirect' })),
        'session',
      ],
      ['401', () => Promise.resolve(answer({ status: 401 })), 'session'],
      ['502', () => Promise.resolve(answer({ status: 502 })), 'down'],
      ['200', () => Promise.resolve(answer()), 'down'],
    ] as [string, () => Promise<unknown>, string][])(
      'a probe that %s ends as %s',
      async (_name, probe, expected) => {
        const { request, sent, clock } = board((input) =>
          input === '/trpc/me' ? probe() : Promise.reject(new TypeError('load failed')),
        );

        await request('/trpc/x').catch(() => undefined);
        clock.now = 1000;
        await request('/trpc/x').catch(() => undefined);
        clock.now = 8000;
        await request('/trpc/x').catch(() => undefined);

        expect(states(sent())).toEqual(expected === 'down' ? ['down'] : ['session']);
      },
    );

    it('goes to /trpc/me with redirect manual and is not observed itself', async () => {
      const { request, calls, sent } = board((input) =>
        input === '/trpc/me'
          ? Promise.resolve(answer({ status: 401 }))
          : Promise.reject(new TypeError('load failed')),
      );

      await request('/trpc/x').catch(() => undefined);

      const probes = calls.filter((call) => call.input === '/trpc/me');
      expect(probes).toHaveLength(1);
      expect(probes[0]?.init).toMatchObject({ redirect: 'manual', cache: 'no-store' });
      expect(states(sent())).toEqual(['session']);
    });

    it('starts no probe for an AbortError', async () => {
      const { request, calls, sent } = board(() =>
        Promise.reject(new DOMException('aborted', 'AbortError')),
      );

      await request('/trpc/x').catch(() => undefined);

      expect(calls).toHaveLength(1);
      expect(sent()).toEqual([]);
    });

    it('starts no second probe while one is in flight', async () => {
      let release: (value: unknown) => void = () => undefined;
      const { win, calls } = board((input) =>
        input === '/trpc/me'
          ? new Promise((resolve) => {
              release = resolve;
            })
          : Promise.reject(new TypeError('load failed')),
      );

      await Promise.allSettled([(win.fetch as Fetch)('/trpc/a'), (win.fetch as Fetch)('/trpc/b')]);
      await vi.advanceTimersByTimeAsync(0);
      expect(calls.filter((call) => call.input === '/trpc/me')).toHaveLength(1);
      release(answer());
    });
  });

  describe('the failure threshold', () => {
    const fail = () => Promise.resolve(answer({ status: 503 }));

    it('reports down once, after a third failure 8 s into the run', async () => {
      const { request, sent, clock } = board(fail);

      await request('/trpc/x');
      clock.now = 1000;
      await request('/trpc/x');
      expect(sent()).toEqual([]);

      clock.now = 8000;
      await request('/trpc/x');
      clock.now = 13000;
      await request('/trpc/x');

      expect(states(sent())).toEqual(['down']);
    });

    it('reports ok after a success that follows down', async () => {
      let healthy = false;
      const { request, sent, clock } = board(() =>
        Promise.resolve(answer({ status: healthy ? 200 : 503 })),
      );
      await request('/trpc/x');
      clock.now = 9000;
      await request('/trpc/x');
      healthy = true;
      clock.now = 10000;
      await request('/trpc/x');
      await request('/trpc/x');

      expect(states(sent())).toEqual(['down', 'ok']);
    });

    it('restarts the run after a success', async () => {
      let healthy = false;
      const { request, sent, clock } = board(() =>
        Promise.resolve(answer({ status: healthy ? 200 : 503 })),
      );
      await request('/trpc/x');
      healthy = true;
      clock.now = 4000;
      await request('/trpc/x');
      healthy = false;
      clock.now = 9000;
      await request('/trpc/x');
      clock.now = 10000;
      await request('/trpc/x');

      expect(states(sent())).toEqual(['ok']);
    });

    it('reports session at once, then ok again once signed in', async () => {
      let status = 401;
      const { request, sent } = board(() => Promise.resolve(answer({ status })));

      await request('/trpc/x');
      status = 200;
      await request('/trpc/x');

      expect(states(sent())).toEqual(['session', 'ok']);
    });
  });

  it('sends one ok after the first success and nothing for the next', async () => {
    const { request, sent } = board(() => Promise.resolve(answer()));

    await request('/trpc/x');
    await request('/trpc/x');

    expect(states(sent())).toEqual(['ok']);
  });

  it.each(['home', 'tab'])('also observes in the %s role', async (role) => {
    const { request, sent } = board(() => Promise.resolve(answer()), role);

    await request('/trpc/x');

    expect(states(sent())).toEqual(['ok']);
  });

  describe('transparency', () => {
    it('hands the board the very promise the original returned', () => {
      const original = Promise.resolve(answer());
      const { win } = board(() => original);

      expect((win.fetch as Fetch)('/trpc/x')).toBe(original);
    });

    it('still rejects for the board with the same error', async () => {
      const failure = new TypeError('load failed');
      const { win } = board((input) =>
        input === '/trpc/me' ? Promise.reject(new TypeError('probe')) : Promise.reject(failure),
      );

      await expect((win.fetch as Fetch)('/trpc/x')).rejects.toBe(failure);
    });

    it('leaves an unobserved request alone', () => {
      const original = Promise.resolve(answer());
      const { win, calls } = board(() => original);

      expect((win.fetch as Fetch)('/api/x')).toBe(original);
      expect(calls).toHaveLength(1);
    });
  });
});
