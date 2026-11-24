import { invoke as tauriInvoke } from '@tauri-apps/api/core';

import { toBoardError, type Invoke } from '../transport.ts';

export const SIGN_IN_POLL_STATUSES = ['pending', 'signed-in', 'no-flow'] as const;
export type SignInPollStatus = (typeof SIGN_IN_POLL_STATUSES)[number];

export interface SignInStarted {
  readonly code: string;
}

export interface SignInApi {
  start(): Promise<SignInStarted>;
  poll(code: string): Promise<SignInPollStatus>;
  cancel(code: string): Promise<void>;
  signOut(): Promise<void>;
}

function isPollStatus(value: unknown): value is SignInPollStatus {
  return SIGN_IN_POLL_STATUSES.some((status) => status === value);
}

export function createSignInApi(invoke: Invoke): SignInApi {
  const send = (command: string, args?: Record<string, unknown>): Promise<unknown> =>
    invoke(command, args).catch((rejection: unknown) => {
      throw toBoardError(rejection);
    });

  return {
    async start() {
      const answer = await send('sign_in_start');
      if (typeof answer !== 'object' || answer === null || !('code' in answer)) {
        throw new TypeError('sign_in_start answered something else than a code');
      }
      const { code } = answer;
      if (typeof code !== 'string') {
        throw new TypeError('sign_in_start answered something else than a code');
      }
      return { code };
    },
    async poll(code) {
      const answer = await send('sign_in_poll', { code });
      if (!isPollStatus(answer)) {
        throw new TypeError('sign_in_poll answered an unknown status');
      }
      return answer;
    },
    async cancel(code) {
      await send('sign_in_cancel', { code });
    },
    async signOut() {
      const answer = await send('sign_out');
      if (answer !== 'signed-out') {
        throw new TypeError('sign_out answered something else than signed-out');
      }
    },
  };
}

export const signInApi: SignInApi = createSignInApi(tauriInvoke);
