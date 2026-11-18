(() => {
  const live = new Map();
  const keys = new WeakMap();
  let untagged = 0;

  function emit(notification, type) {
    const event = new Event(type);
    notification.dispatchEvent(event);
    notification[`on${type}`]?.(event);
  }

  class Notification extends EventTarget {
    static get permission() {
      return 'granted';
    }

    static requestPermission(callback) {
      if (typeof callback === 'function') callback('granted');
      return Promise.resolve('granted');
    }

    constructor(title, options = {}) {
      if (arguments.length === 0) {
        throw new TypeError("Failed to construct 'Notification': 1 argument required.");
      }
      super();
      this.title = String(title);
      this.body = String(options.body ?? '');
      this.tag = String(options.tag ?? '');
      this.data = options.data ?? null;
      this.onclick = null;
      this.onerror = null;

      const key = this.tag === '' ? `agent-board-client:${++untagged}` : this.tag;
      keys.set(this, key);
      live.set(key, this);

      window.__TAURI_INTERNALS__
        .invoke('notify', { title: this.title, body: this.body, tag: key })
        .catch(() => {
          this.close();
          emit(this, 'error');
        });
    }

    close() {
      const key = keys.get(this);
      if (live.get(key) === this) live.delete(key);
    }
  }

  function activateNotification(key) {
    const notification = live.get(key);
    if (notification) emit(notification, 'click');
  }

  Object.defineProperty(window, 'Notification', {
    value: Notification,
    writable: true,
    configurable: true,
  });
  Object.defineProperty(window, '__agentBoardClient', {
    value: Object.freeze({ activateNotification }),
  });
})();
