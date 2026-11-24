import { Button, Center, Loader, Stack, Text, Title } from '@mantine/core';

import type { BoardStateName } from '../state.ts';

export type ScreenState =
  Exclude<BoardStateName, 'ready' | 'signed-out'> | 'loading' | 'unavailable';

interface Copy {
  readonly title: string;
  readonly text?: string;
  readonly retry?: boolean;
}

const COPY: Readonly<Record<Exclude<ScreenState, 'loading'>, Copy>> = {
  'upgrade-required': {
    title: 'This app needs an update',
    text: 'This board needs a newer version of the app.',
  },
  'board-too-old': {
    title: 'This board is too old',
    text: "This board does not support the app's own pages yet.",
  },
  offline: {
    title: 'The board cannot be reached',
    text: 'Check the connection; the page retries by itself.',
    retry: true,
  },
  server: {
    title: 'The board returned an error',
    text: 'Try again in a moment.',
    retry: true,
  },
  'board-only': { title: 'Opening the board…' },
  unavailable: { title: 'This page lost its server', text: 'Close the tab.' },
};

export function StateScreen({ state, onRetry }: { state: ScreenState; onRetry?: () => void }) {
  if (state === 'loading') {
    return (
      <Center h="100vh">
        <Loader />
      </Center>
    );
  }

  const copy = COPY[state];
  return (
    <Center h="100vh">
      <Stack align="center" gap="xs" p="md">
        <Title order={2}>{copy.title}</Title>
        {copy.text && <Text>{copy.text}</Text>}
        {copy.retry && onRetry && <Button onClick={onRetry}>Try again</Button>}
      </Stack>
    </Center>
  );
}
