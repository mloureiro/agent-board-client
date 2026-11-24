import { Alert, Button, Center, Group, Stack, Text, Title } from '@mantine/core';
import { useQueryClient } from '@tanstack/react-query';
import { useCallback, useEffect, useRef, useState } from 'react';

import { BOARD_STATE_KEY } from '../state.ts';
import { signInApi, type SignInApi } from './api.ts';
import { startPollLoop, type PollLoop } from './pollLoop.ts';
import { stepAfter, stepAfterStartFailure, type SignInStep } from './steps.ts';

type Phase =
  | { readonly kind: 'idle' }
  | { readonly kind: 'starting' }
  | { readonly kind: 'waiting'; readonly code: string }
  | { readonly kind: 'failed'; readonly message: string };

const IDLE: Phase = { kind: 'idle' };

export function SignInScreen({
  serverName,
  api = signInApi,
}: {
  serverName: string;
  api?: SignInApi;
}) {
  const queryClient = useQueryClient();
  const [phase, setPhase] = useState<Phase>(IDLE);
  const loop = useRef<PollLoop | undefined>(undefined);
  const code = phase.kind === 'waiting' ? phase.code : undefined;

  const rereadState = useCallback(() => {
    void queryClient.invalidateQueries({ queryKey: BOARD_STATE_KEY });
  }, [queryClient]);

  const apply = useCallback(
    (step: SignInStep) => {
      if (step.kind === 'failed') {
        setPhase({ kind: 'failed', message: step.message });
        return;
      }
      setPhase(IDLE);
      if (step.kind === 'reread-state' || step.rereadState) {
        rereadState();
      }
    },
    [rereadState],
  );

  useEffect(() => {
    if (code === undefined) {
      return;
    }
    const running = startPollLoop({
      poll: () => api.poll(code),
      onSettled: (outcome) => {
        apply(stepAfter(outcome));
      },
    });
    loop.current = running;
    const onVisible = () => {
      if (document.visibilityState === 'visible') {
        running.check();
      }
    };
    const onFocus = () => {
      running.check();
    };
    window.addEventListener('focus', onFocus);
    document.addEventListener('visibilitychange', onVisible);
    return () => {
      running.stop();
      loop.current = undefined;
      window.removeEventListener('focus', onFocus);
      document.removeEventListener('visibilitychange', onVisible);
    };
  }, [api, apply, code]);

  const start = () => {
    setPhase({ kind: 'starting' });
    api.start().then(
      (started) => {
        setPhase({ kind: 'waiting', code: started.code });
      },
      (error: unknown) => {
        apply(stepAfterStartFailure(error));
      },
    );
  };

  const cancel = (waitingCode: string) => {
    setPhase(IDLE);
    api.cancel(waitingCode).catch((error: unknown) => {
      console.error('The sign-in could not be cancelled', error);
    });
  };

  return (
    <Center h="100vh">
      <Stack align="center" gap="md" p="md" maw={480}>
        <Title order={2}>Sign in to {serverName}</Title>
        {phase.kind === 'waiting' ? (
          <>
            <Title order={1} ff="monospace" lts="0.1em" component="output">
              {phase.code}
            </Title>
            <Text ta="center">Approve in the browser only if it shows the same code.</Text>
            <Text role="status" c="dimmed">
              Waiting for approval…
            </Text>
            <Group>
              <Button
                onClick={() => {
                  loop.current?.check();
                }}
              >
                Check now
              </Button>
              <Button
                variant="subtle"
                onClick={() => {
                  cancel(phase.code);
                }}
              >
                Cancel
              </Button>
            </Group>
          </>
        ) : (
          <>
            {phase.kind === 'failed' ? (
              <Alert color="red" role="alert">
                {phase.message}
              </Alert>
            ) : (
              <Text ta="center">
                Sign in on the board&apos;s own page in your browser. This app keeps the session in
                your keychain.
              </Text>
            )}
            <Button loading={phase.kind === 'starting'} onClick={start}>
              {phase.kind === 'failed' ? 'Start again' : 'Sign in with your browser'}
            </Button>
          </>
        )}
      </Stack>
    </Center>
  );
}
