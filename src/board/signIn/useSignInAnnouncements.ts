import { useQueryClient } from '@tanstack/react-query';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { useEffect } from 'react';

import { BOARD_STATE_KEY } from '../state.ts';

export function useSignInAnnouncements(): void {
  const queryClient = useQueryClient();

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .listen('signed-in', () => {
        void queryClient.invalidateQueries({ queryKey: BOARD_STATE_KEY });
      })
      .then((handler) => {
        if (cancelled) {
          handler();
        } else {
          unlisten = handler;
        }
      })
      .catch((error: unknown) => {
        console.error('The sign-in listener could not be registered', error);
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [queryClient]);
}
