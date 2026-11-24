import { Stack, Text, Title } from '@mantine/core';
import { useLocation } from 'react-router-dom';
import { isNativePath, NATIVE_PAGES } from './pages.ts';

export function PlaceholderPage() {
  const location = useLocation();

  if (!isNativePath(location.pathname, NATIVE_PAGES)) {
    return null;
  }

  const server = new URLSearchParams(window.location.search).get('server') ?? '';
  return (
    <Stack p="md">
      <Title order={2}>{server}</Title>
      <Text>Loading…</Text>
    </Stack>
  );
}
