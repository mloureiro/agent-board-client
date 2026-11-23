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

  const pending = [];
  let draining = false;

  function drain() {
    try {
      win.location.href = pending.shift();
    } finally {
      if (pending.length > 0) {
        win.setTimeout(drain, 0);
      } else {
        draining = false;
      }
    }
  }

  function send(href) {
    pending.push(href);
    if (!draining) {
      draining = true;
      win.setTimeout(drain, 0);
    }
  }

  function sendPath(verb, url) {
    send('agent-board-client:' + verb + '?path=' + encodeURIComponent(url.pathname + url.search));
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
        sendPath('open', url);
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
        sendPath('route', url);
        return true;
      },
      after(target) {
        const url = resolve(target);
        if (url !== null) {
          sendPath('route', url);
        }
      },
    },
  };

  for (const method of ['pushState', 'replaceState']) {
    wrap(method, handlers[role]);
  }

  win.addEventListener(
    'keydown',
    function (event) {
      if (event.key !== 'Tab' || !event.ctrlKey || event.metaKey) {
        return;
      }
      event.preventDefault();
      send('agent-board-client:cycle?dir=' + (event.shiftKey ? 'previous' : 'next'));
    },
    true,
  );

  if (role === 'tab') {
    win.addEventListener('popstate', function () {
      sendPath('route', win.location);
    });
  }

  const MAX_TITLE = 200;
  const MAX_BODY = 500;
  const MAX_TAG = 200;

  function cut(value, max) {
    const points = Array.from(value);
    return points.length <= max ? value : points.slice(0, max - 1).join('') + '\u2026';
  }

  function notify(title, options) {
    try {
      const settings = options !== null && typeof options === 'object' ? options : {};
      const data = settings.data !== null && typeof settings.data === 'object' ? settings.data : {};
      let href =
        'agent-board-client:notify?title=' +
        encodeURIComponent(cut(String(title), MAX_TITLE)) +
        '&body=' +
        encodeURIComponent(cut(settings.body === undefined ? '' : String(settings.body), MAX_BODY));
      if (typeof settings.tag === 'string' && Array.from(settings.tag).length <= MAX_TAG) {
        href += '&tag=' + encodeURIComponent(settings.tag);
      }
      if (typeof data.path === 'string') {
        href += '&path=' + encodeURIComponent(data.path);
      }
      send(href);
    } catch {
      return;
    }
  }

  function install(forward) {
    function StandInNotification(title, options) {
      const target = new win.EventTarget();
      target.close = function () {};
      forward(title, options);
      return target;
    }
    Object.defineProperty(StandInNotification, 'permission', {
      get() {
        return 'granted';
      },
    });
    StandInNotification.requestPermission = function (callback) {
      if (typeof callback === 'function') {
        callback('granted');
      }
      return Promise.resolve('granted');
    };
    win.Notification = StandInNotification;

    if (win.ServiceWorkerRegistration) {
      win.ServiceWorkerRegistration.prototype.showNotification = function (title, options) {
        forward(title, options);
        return Promise.resolve(undefined);
      };
    }
  }

  install(role === 'home' ? notify : function () {});
});
