import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { BoardError } from '../transport.ts';
import { createSignInApi } from './api.ts';
import { SIGN_IN_POLL_MS, startPollLoop, type PollOutcome } from './pollLoop.ts';

interface Deferred {
  resolve(value: unknown): void;
  reject(reason: unknown): void;
}

function fakeBoard() {
  const pending: Deferred[] = [];
  const invoke = vi.fn(
    () =>
      new Promise<unknown>((resolve, reject) => {
        pending.push({ resolve, reject });
      }),
  );
  const next = (): Deferred => {
    const deferred = pending.shift();
    if (!deferred) {
      throw new Error('no call is in flight');
    }
    return deferred;
  };
  return { invoke, next };
}

function setup() {
  const board = fakeBoard();
  const api = createSignInApi(board.invoke);
  const settled: PollOutcome[] = [];
  const loop = startPollLoop({
    poll: () => api.poll('ABCD-EF23'),
    onSettled: (outcome) => settled.push(outcome),
  });
  return { ...board, loop, settled };
}

async function answer(deferred: Deferred, value: unknown) {
  deferred.resolve(value);
  await vi.advanceTimersByTimeAsync(0);
}

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('startPollLoop', () => {
  it('polls first after the interval, with the code', async () => {
    const { invoke } = setup();

    await vi.advanceTimersByTimeAsync(SIGN_IN_POLL_MS - 1);
    expect(invoke).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    expect(invoke).toHaveBeenCalledExactlyOnceWith('sign_in_poll', { code: 'ABCD-EF23' });
  });

  it('keeps one call in flight at a time', async () => {
    const { invoke, loop } = setup();
    await vi.advanceTimersByTimeAsync(SIGN_IN_POLL_MS);

    loop.check();
    loop.check();
    await vi.advanceTimersByTimeAsync(SIGN_IN_POLL_MS * 3);

    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it('polls again an interval after the answer, not after the request', async () => {
    const { invoke, next } = setup();
    await vi.advanceTimersByTimeAsync(SIGN_IN_POLL_MS);
    await vi.advanceTimersByTimeAsync(5_000);

    await answer(next(), 'pending');
    await vi.advanceTimersByTimeAsync(SIGN_IN_POLL_MS - 1);
    expect(invoke).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(1);
    expect(invoke).toHaveBeenCalledTimes(2);
  });

  it('polls at once on check() and restarts the timer', async () => {
    const { invoke, next, loop } = setup();
    await vi.advanceTimersByTimeAsync(SIGN_IN_POLL_MS - 500);

    loop.check();
    expect(invoke).toHaveBeenCalledTimes(1);
    await answer(next(), 'pending');

    await vi.advanceTimersByTimeAsync(SIGN_IN_POLL_MS - 1);
    expect(invoke).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(invoke).toHaveBeenCalledTimes(2);
  });

  it.each(['signed-in', 'no-flow'] as const)('stops once on %s', async (status) => {
    const { invoke, next, settled } = setup();
    await vi.advanceTimersByTimeAsync(SIGN_IN_POLL_MS);

    await answer(next(), status);
    await vi.advanceTimersByTimeAsync(10_000);

    expect(settled).toEqual([{ kind: status }]);
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it('stops on an error and hands it over', async () => {
    const { invoke, next, settled } = setup();
    await vi.advanceTimersByTimeAsync(SIGN_IN_POLL_MS);

    next().reject({ class: 'server', message: 'This sign-in has expired. Start again.' });
    await vi.advanceTimersByTimeAsync(10_000);

    expect(settled).toHaveLength(1);
    const [outcome] = settled;
    expect(outcome).toMatchObject({ kind: 'failed' });
    const error = outcome?.kind === 'failed' ? outcome.error : undefined;
    expect(error).toBeInstanceOf(BoardError);
    expect(error).toMatchObject({ message: 'This sign-in has expired. Start again.' });
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it('ignores a call still in flight when stopped', async () => {
    const { invoke, next, settled, loop } = setup();
    await vi.advanceTimersByTimeAsync(SIGN_IN_POLL_MS);

    loop.stop();
    await answer(next(), 'signed-in');
    loop.check();
    await vi.advanceTimersByTimeAsync(10_000);

    expect(settled).toEqual([]);
    expect(invoke).toHaveBeenCalledTimes(1);
  });
});
