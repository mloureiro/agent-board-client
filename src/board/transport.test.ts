import { describe, expect, it, vi } from 'vitest';

import {
  BOARD_ERROR_CLASSES,
  BoardError,
  createBoardTransport,
  toBoardError,
} from './transport.ts';

describe('toBoardError', () => {
  it.each(BOARD_ERROR_CLASSES)('turns the %s class into a BoardError', (errorClass) => {
    const error = toBoardError({ class: errorClass, message: 'Said by the board.' });

    expect(error).toBeInstanceOf(BoardError);
    expect(error).toMatchObject({ class: errorClass, message: 'Said by the board.' });
  });

  it.each(BOARD_ERROR_CLASSES)(
    'gives the %s class a sentence when the board sent none',
    (errorClass) => {
      const error = toBoardError({ class: errorClass });

      expect(error).toBeInstanceOf(BoardError);
      expect((error as BoardError).message).not.toBe('');
    },
  );

  it.each([
    ['a Tauri refusal', 'command board_call not allowed'],
    ['a client error', { kind: 'notFound' }],
    ['an unknown class', { class: 'nope' }],
    ['null', null],
  ])('returns %s unchanged', (_name, rejection) => {
    expect(toBoardError(rejection)).toBe(rejection);
  });
});

describe('createBoardTransport', () => {
  it('passes a query to board_call', async () => {
    const invoke = vi.fn().mockResolvedValue({ id: 'u' });

    const answer = await createBoardTransport(invoke).query('me', undefined);

    expect(answer).toEqual({ id: 'u' });
    expect(invoke).toHaveBeenCalledWith('board_call', {
      kind: 'query',
      path: 'me',
      input: undefined,
    });
  });

  it('passes a mutation to board_call', async () => {
    const invoke = vi.fn().mockResolvedValue(null);

    await createBoardTransport(invoke).mutate('tasks.stop', { id: 't' });

    expect(invoke).toHaveBeenCalledWith('board_call', {
      kind: 'mutation',
      path: 'tasks.stop',
      input: { id: 't' },
    });
  });

  it('rejects with a BoardError when the board refuses', async () => {
    const invoke = vi.fn().mockRejectedValue({ class: 'signed-out' });

    await expect(createBoardTransport(invoke).query('me', undefined)).rejects.toMatchObject({
      class: 'signed-out',
    });
  });

  it('does not invoke when the signal is already aborted', async () => {
    const invoke = vi.fn();
    const controller = new AbortController();
    controller.abort();

    await expect(
      createBoardTransport(invoke).query('me', undefined, controller.signal),
    ).rejects.toMatchObject({ name: 'AbortError' });
    expect(invoke).not.toHaveBeenCalled();
  });

  it('rejects with AbortError when the signal aborts while the call is pending', async () => {
    const invoke = vi.fn().mockReturnValue(new Promise(() => undefined));
    const controller = new AbortController();

    const pending = createBoardTransport(invoke).query('me', undefined, controller.signal);
    controller.abort();

    await expect(pending).rejects.toMatchObject({ name: 'AbortError' });
  });
});
