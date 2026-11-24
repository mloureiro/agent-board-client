import '@mantine/core/styles.css';
import { MantineProvider } from '@mantine/core';
import { MutationCache, QueryCache, QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { createHashRouter, RouterProvider } from 'react-router-dom';
import { BoardGate } from './signIn/BoardGate.tsx';
import { isSignedOut } from './signIn/steps.ts';
import { BOARD_STATE_KEY } from './state.ts';

const router = createHashRouter([
  {
    path: '*',
    lazy: async () => ({
      Component: (await import('./PlaceholderPage.tsx')).PlaceholderPage,
    }),
  },
]);
const queryClient: QueryClient = new QueryClient({
  queryCache: new QueryCache({ onError: returnToSignIn }),
  mutationCache: new MutationCache({ onError: returnToSignIn }),
});

function returnToSignIn(error: unknown) {
  if (isSignedOut(error)) {
    void queryClient.invalidateQueries({ queryKey: BOARD_STATE_KEY });
  }
}

const root = document.getElementById('root');
if (!root) {
  throw new Error('board.html has no #root element');
}

createRoot(root).render(
  <StrictMode>
    <MantineProvider defaultColorScheme="auto">
      <QueryClientProvider client={queryClient}>
        <BoardGate>
          <RouterProvider router={router} />
        </BoardGate>
      </QueryClientProvider>
    </MantineProvider>
  </StrictMode>,
);
