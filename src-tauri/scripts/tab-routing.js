(function (role, win) {
  if (role !== 'home' && role !== 'tab') {
    return;
  }

  function classify(pathname) {
    const path = pathname.length > 1 && pathname.endsWith('/') ? pathname.slice(0, -1) : pathname;
    if (path === '/') {
      return 'home';
    }
    if (
      path === '/backlog' ||
      path === '/schedules' ||
      path === '/closed' ||
      path === '/settings' ||
      path.startsWith('/settings/')
    ) {
      return 'page';
    }
    if (path.startsWith('/t/') && path.length > 3 && !path.includes('/', 3)) {
      return 'page';
    }
    return 'other';
  }

  function resolve(target) {
    if (target === undefined || target === null) {
      return null;
    }
    const url = new win.URL(String(target), win.location.href);
    return url.origin === win.location.origin ? url : null;
  }

  function send(verb, url) {
    win.location.href =
      'agent-board-client:' + verb + '?path=' + encodeURIComponent(url.pathname + url.search);
  }

  function wrap(method, handler) {
    const original = win.history[method];
    win.history[method] = function (...args) {
      try {
        if (handler.before(args[2])) {
          return undefined;
        }
      } catch {
        return original.apply(win.history, args);
      }
      const result = original.apply(win.history, args);
      try {
        handler.after(args[2]);
      } catch {
        return result;
      }
      return result;
    };
  }

  const handlers = {
    home: {
      before(target) {
        const url = resolve(target);
        if (url === null || classify(url.pathname) !== 'page') {
          return false;
        }
        send('open', url);
        return true;
      },
      after() {},
    },
    tab: {
      before(target) {
        const url = resolve(target);
        if (url === null || classify(url.pathname) !== 'home') {
          return false;
        }
        send('route', url);
        return true;
      },
      after(target) {
        const url = resolve(target);
        if (url !== null) {
          send('route', url);
        }
      },
    },
  };

  for (const method of ['pushState', 'replaceState']) {
    wrap(method, handlers[role]);
  }

  if (role !== 'tab') {
    return;
  }

  win.addEventListener('popstate', function () {
    send('route', win.location);
  });

  function SilentNotification() {
    const target = new win.EventTarget();
    target.close = function () {};
    return target;
  }
  Object.defineProperty(SilentNotification, 'permission', {
    get() {
      return 'granted';
    },
  });
  SilentNotification.requestPermission = function (callback) {
    if (typeof callback === 'function') {
      callback('granted');
    }
    return Promise.resolve('granted');
  };
  win.Notification = SilentNotification;
});
