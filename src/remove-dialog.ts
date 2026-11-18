import type { Server } from './backend';

const DELETE_DATA_BY_DEFAULT = true;
const CONFIRMED = 'remove';

export interface RemovalChoice {
  readonly deleteData: boolean;
}

export interface RemoveDialog {
  readonly dialog: HTMLDialogElement;
  readonly title: HTMLElement;
  readonly deleteData: HTMLInputElement;
}

export function askToRemove(
  { dialog, title, deleteData }: RemoveDialog,
  server: Server,
): Promise<RemovalChoice | null> {
  title.textContent = `Remove ${server.name}?`;
  deleteData.checked = DELETE_DATA_BY_DEFAULT;
  dialog.returnValue = '';

  return new Promise((resolve) => {
    dialog.addEventListener(
      'close',
      () => {
        resolve(dialog.returnValue === CONFIRMED ? { deleteData: deleteData.checked } : null);
      },
      { once: true },
    );
    dialog.showModal();
  });
}
