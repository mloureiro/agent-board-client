import '@mantine/core/styles.css';
import { MantineProvider } from '@mantine/core';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { createHashRouter, RouterProvider } from 'react-router-dom';
import { useBoardState } from './state.ts';

const router = createHashRouter([
  {
    path: '*',
    lazy: async () => ({
      Component: (await import('./PlaceholderPage.tsx')).PlaceholderPage,
    }),
  },
]);
const queryClient = new QueryClient();

function LiveUpdates() {
  useBoardState();
  return null;
}

const root = document.getElementById('root');
if (!root) {
  throw new Error('board.html has no #root element');
}

createRoot(root).render(
  <StrictMode>
    <MantineProvider defaultColorScheme="auto">
      <QueryClientProvider client={queryClient}>
        <LiveUpdates />
        <RouterProvider router={router} />
      </QueryClientProvider>
    </MantineProvider>
  </StrictMode>,
);
