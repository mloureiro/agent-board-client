import { Button, Text } from '@mantine/core';
import { useMutation, useQueryClient } from '@tanstack/react-query';

import { BOARD_STATE_KEY } from '../state.ts';
import { signInApi, type SignInApi } from './api.ts';

export function SignOutButton({ api = signInApi }: { api?: SignInApi }) {
  const queryClient = useQueryClient();
  const signOut = useMutation({
    mutationFn: () => api.signOut(),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: BOARD_STATE_KEY }),
  });

  return (
    <>
      {signOut.isError && (
        <Text size="xs" c="red">
          {signOut.error.message}
        </Text>
      )}
      <Button
        variant="subtle"
        size="compact-sm"
        loading={signOut.isPending}
        onClick={() => {
          signOut.mutate();
        }}
      >
        Sign out
      </Button>
    </>
  );
}
