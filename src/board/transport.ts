import type { BoardTransport } from '@agent-board/shared/ui';
import { invoke as tauriInvoke } from '@tauri-apps/api/core';

export type Invoke = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

export const BOARD_ERROR_CLASSES = [
  'offline',
  'signed-out',
  'upgrade-required',
  'board-too-old',
  'server',
] as const;
export type BoardErrorClass = (typeof BOARD_ERROR_CLASSES)[number];

const CLASS_MESSAGES: Readonly<Record<BoardErrorClass, string>> = {
  offline: 'The board cannot be reached.',
  'signed-out': 'You are signed out of this board.',
  'upgrade-required': 'This app is too old for this board. Update it.',
  'board-too-old': 'This board is too old for this app.',
  server: 'The board returned an error.',
};

export class BoardError extends Error {
  readonly class: BoardErrorClass;

  constructor(errorClass: BoardErrorClass, message: string) {
    super(message);
    this.name = 'BoardError';
    this.class = errorClass;
  }
}

function isBoardErrorClass(value: unknown): value is BoardErrorClass {
  return BOARD_ERROR_CLASSES.some((errorClass) => errorClass === value);
}

export function toBoardError(rejection: unknown): unknown {
  if (typeof rejection !== 'object' || rejection === null || !('class' in rejection)) {
    return rejection;
  }
  const errorClass = rejection.class;
  if (!isBoardErrorClass(errorClass)) {
    return rejection;
  }
  const message = 'message' in rejection ? rejection.message : undefined;
  return new BoardError(
    errorClass,
    typeof message === 'string' && message !== '' ? message : CLASS_MESSAGES[errorClass],
  );
}

function abortError(): DOMException {
  return new DOMException('Aborted', 'AbortError');
}

// The native call cannot be cancelled; it ends within its own timeout.
function raceAbort<T>(work: Promise<T>, signal: AbortSignal): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const onAbort = () => {
      reject(abortError());
    };
    signal.addEventListener('abort', onAbort, { once: true });
    work.then(resolve, reject).finally(() => {
      signal.removeEventListener('abort', onAbort);
    });
  });
}

export function createBoardTransport(invoke: Invoke): BoardTransport {
  const send = <T>(
    kind: 'query' | 'mutation',
    path: string,
    input: unknown,
    signal?: AbortSignal,
  ): Promise<T> => {
    if (signal?.aborted) {
      return Promise.reject(abortError());
    }
    const work = invoke('board_call', { kind, path, input }).then(
      (answer) => answer as T,
      (rejection: unknown) => {
        throw toBoardError(rejection);
      },
    );
    return signal ? raceAbort(work, signal) : work;
  };

  return {
    query: (path, input, signal) => send('query', path, input, signal),
    mutate: (path, input) => send('mutation', path, input),
  };
}

export const boardTransport = createBoardTransport(tauriInvoke);
