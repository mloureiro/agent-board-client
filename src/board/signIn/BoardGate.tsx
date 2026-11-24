import { Group } from '@mantine/core';
import { useQueryClient } from '@tanstack/react-query';
import type { ReactNode } from 'react';

import { BOARD_STATE_KEY, useBoardState } from '../state.ts';
import { SignInScreen } from './SignInScreen.tsx';
import { SignOutButton } from './SignOutButton.tsx';
import { StateScreen } from './StateScreen.tsx';
import { useSignInAnnouncements } from './useSignInAnnouncements.ts';

export function BoardGate({ children }: { children: ReactNode }) {
  const queryClient = useQueryClient();
  const { view, isError } = useBoardState();
  useSignInAnnouncements();

  if (isError) {
    return <StateScreen state="unavailable" />;
  }
  if (!view) {
    return <StateScreen state="loading" />;
  }
  if (view.state === 'signed-out') {
    return <SignInScreen serverName={view.serverName} />;
  }
  if (view.state === 'ready') {
    return (
      <>
        <Group justify="flex-end" p="xs">
          {view.signedIn && <SignOutButton />}
        </Group>
        {children}
      </>
    );
  }
  return (
    <StateScreen
      state={view.state}
      onRetry={() => {
        void queryClient.invalidateQueries({ queryKey: BOARD_STATE_KEY });
      }}
    />
  );
}
