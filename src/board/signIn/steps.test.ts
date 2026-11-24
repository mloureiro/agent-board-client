import { describe, expect, it } from 'vitest';

import { BoardError, type BoardErrorClass } from '../transport.ts';
import { isSignedOut, stepAfter, stepAfterStartFailure, type SignInStep } from './steps.ts';

const failed = (error: unknown) => ({ kind: 'failed', error }) as const;
const boardError = (errorClass: BoardErrorClass, message = 'Said by the board.') =>
  new BoardError(errorClass, message);

describe('stepAfter', () => {
  it.each<[string, Parameters<typeof stepAfter>[0], SignInStep]>([
    ['signed-in', { kind: 'signed-in' }, { kind: 'reread-state' }],
    ['no-flow', { kind: 'no-flow' }, { kind: 'idle', rereadState: true }],
    ['offline', failed(boardError('offline')), { kind: 'reread-state' }],
    ['upgrade-required', failed(boardError('upgrade-required')), { kind: 'reread-state' }],
    ['signed-out', failed(boardError('signed-out')), { kind: 'reread-state' }],
    [
      'a server error',
      failed(boardError('server', 'This sign-in has expired. Start again.')),
      { kind: 'failed', message: 'This sign-in has expired. Start again.' },
    ],
    [
      'anything else',
      failed(new Error('boom')),
      { kind: 'failed', message: 'The sign-in could not be checked.' },
    ],
  ])('maps %s', (_name, outcome, expected) => {
    expect(stepAfter(outcome)).toEqual(expected);
  });
});

describe('stepAfterStartFailure', () => {
  it('names a start in the generic message', () => {
    expect(stepAfterStartFailure('denied')).toEqual({
      kind: 'failed',
      message: 'The sign-in could not be started.',
    });
  });

  it('keeps the board message of a server error', () => {
    expect(stepAfterStartFailure(boardError('server'))).toEqual({
      kind: 'failed',
      message: 'Said by the board.',
    });
  });

  it('sends an offline board to the state screen', () => {
    expect(stepAfterStartFailure(boardError('offline'))).toEqual({ kind: 'reread-state' });
  });
});

describe('isSignedOut', () => {
  it.each([
    ['a signed-out BoardError', boardError('signed-out'), true],
    ['an offline BoardError', boardError('offline'), false],
    ['a plain Error', new Error('signed-out'), false],
  ])('is right for %s', (_name, error, expected) => {
    expect(isSignedOut(error)).toBe(expected);
  });
});
