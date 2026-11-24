import { BoardError } from '../transport.ts';
import type { PollOutcome } from './pollLoop.ts';

export type SignInStep =
  | { readonly kind: 'reread-state' }
  | { readonly kind: 'idle'; readonly rereadState: boolean }
  | { readonly kind: 'failed'; readonly message: string };

const REREAD_CLASSES: readonly string[] = ['offline', 'upgrade-required', 'signed-out'];

function stepAfterFailure(error: unknown, generic: string): SignInStep {
  if (error instanceof BoardError) {
    if (REREAD_CLASSES.includes(error.class)) {
      return { kind: 'reread-state' };
    }
    if (error.class === 'server') {
      return { kind: 'failed', message: error.message };
    }
  }
  return { kind: 'failed', message: generic };
}

export function stepAfter(outcome: PollOutcome): SignInStep {
  switch (outcome.kind) {
    case 'signed-in':
      return { kind: 'reread-state' };
    case 'no-flow':
      return { kind: 'idle', rereadState: true };
    case 'failed':
      return stepAfterFailure(outcome.error, 'The sign-in could not be checked.');
  }
}

export function stepAfterStartFailure(error: unknown): SignInStep {
  return stepAfterFailure(error, 'The sign-in could not be started.');
}

export function isSignedOut(error: unknown): boolean {
  return error instanceof BoardError && error.class === 'signed-out';
}
