import '@mantine/core/styles.css';
import { MantineProvider } from '@mantine/core';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { createHashRouter, RouterProvider } from 'react-router-dom';

const router = createHashRouter([
  {
    path: '*',
    lazy: async () => ({
      Component: (await import('./PlaceholderPage.tsx')).PlaceholderPage,
    }),
  },
]);
const queryClient = new QueryClient();

const root = document.getElementById('root');
if (!root) {
  throw new Error('board.html has no #root element');
}

createRoot(root).render(
  <StrictMode>
    <MantineProvider defaultColorScheme="auto">
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
      </QueryClientProvider>
    </MantineProvider>
  </StrictMode>,
);
