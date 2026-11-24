import type { SignInPollStatus } from './api.ts';

export const SIGN_IN_POLL_MS = 2_000;

export type PollOutcome =
  | { readonly kind: 'signed-in' }
  | { readonly kind: 'no-flow' }
  | { readonly kind: 'failed'; readonly error: unknown };

export interface PollLoop {
  check(): void;
  stop(): void;
}

export function startPollLoop(options: {
  poll: () => Promise<SignInPollStatus>;
  onSettled: (outcome: PollOutcome) => void;
  intervalMs?: number;
}): PollLoop {
  const { poll, onSettled, intervalMs = SIGN_IN_POLL_MS } = options;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let inFlight = false;
  let live = true;

  const clearTimer = () => {
    if (timer !== undefined) {
      clearTimeout(timer);
      timer = undefined;
    }
  };

  const settle = (outcome: PollOutcome) => {
    live = false;
    clearTimer();
    onSettled(outcome);
  };

  const run = () => {
    timer = undefined;
    if (!live || inFlight) {
      return;
    }
    inFlight = true;
    poll().then(
      (status) => {
        inFlight = false;
        if (!live) {
          return;
        }
        if (status === 'pending') {
          timer = setTimeout(run, intervalMs);
        } else {
          settle({ kind: status });
        }
      },
      (error: unknown) => {
        inFlight = false;
        if (live) {
          settle({ kind: 'failed', error });
        }
      },
    );
  };

  timer = setTimeout(run, intervalMs);

  return {
    check() {
      if (!live || inFlight) {
        return;
      }
      clearTimer();
      run();
    },
    stop() {
      live = false;
      clearTimer();
    },
  };
}
