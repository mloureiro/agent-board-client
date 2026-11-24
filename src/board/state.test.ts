// @vitest-environment jsdom
import { QueryClient, QueryClientProvider, useQuery } from '@tanstack/react-query';
import { renderHook, waitFor } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { refetchIntervalFor, SLOW_POLL_MS, useBoardState } from './state.ts';

const { invoke, listen } = vi.hoisted(() => ({
  invoke: vi.fn<(command: string) => Promise<unknown>>(),
  listen: vi.fn<(name: string, handler: (event: unknown) => void) => Promise<() => void>>(),
}));

vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/api/webview', () => ({ getCurrentWebview: () => ({ listen }) }));

const view = { state: 'ready', serverName: 'Board', eventsUp: true };

function handlerOf(name: string): () => void {
  const call = listen.mock.calls.find(([registered]) => registered === name);
  if (!call) {
    throw new Error(`no listener for ${name}`);
  }
  return () => {
    call[1]({ payload: {} });
  };
}

function boardStateCalls(): number {
  return invoke.mock.calls.filter(([command]) => command === 'board_state').length;
}

function render(tasks: () => Promise<string>) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const wrapper = ({ children }: { children: ReactNode }) =>
    createElement(QueryClientProvider, { client }, children);
  return renderHook(
    () => {
      useQuery({ queryKey: ['tasks'], queryFn: tasks });
      return useBoardState();
    },
    { wrapper },
  );
}

describe('useBoardState', () => {
  beforeEach(() => {
    invoke.mockReset().mockResolvedValue(view);
    listen.mockReset();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('reads the board state only after its listeners attach', async () => {
    const attached: (() => void)[] = [];
    listen.mockImplementation(
      () =>
        new Promise<() => void>((resolve) => {
          attached.push(() => {
            resolve(() => undefined);
          });
        }),
    );

    const { result } = render(() => Promise.resolve('tasks'));
    await Promise.resolve();

    expect(boardStateCalls()).toBe(0);

    attached.forEach((resolve) => {
      resolve();
    });

    await waitFor(() => {
      expect(result.current.view).toEqual(view);
    });
    expect(boardStateCalls()).toBe(1);
  });

  it('refreshes the page queries on a board change but not the board state', async () => {
    listen.mockResolvedValue(() => undefined);
    const tasks = vi.fn(() => Promise.resolve('tasks'));

    const { result } = render(tasks);
    await waitFor(() => {
      expect(result.current.view).toEqual(view);
    });
    await waitFor(() => {
      expect(tasks).toHaveBeenCalledTimes(1);
    });

    handlerOf('board-changed')();

    await waitFor(() => {
      expect(tasks).toHaveBeenCalledTimes(2);
    });
    expect(boardStateCalls()).toBe(1);
  });

  it('still reads the board state when the listeners cannot attach', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => undefined);
    listen.mockRejectedValue(new Error('no webview'));

    const { result } = render(() => Promise.resolve('tasks'));

    await waitFor(() => {
      expect(result.current.view).toEqual(view);
    });
    expect(boardStateCalls()).toBe(1);
  });
});

describe('refetchIntervalFor', () => {
  it.each([
    [true, false],
    [false, SLOW_POLL_MS],
  ])('with events up %s polls %s', (eventsUp, expected) => {
    expect(refetchIntervalFor(eventsUp)).toBe(expected);
  });
});
