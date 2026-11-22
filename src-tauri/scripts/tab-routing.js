(function (role, win) {
  if (role !== 'home') {
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

  function swallowed(target) {
    try {
      if (target === undefined || target === null) {
        return false;
      }
      const url = new win.URL(String(target), win.location.href);
      if (url.origin !== win.location.origin || classify(url.pathname) !== 'page') {
        return false;
      }
      win.location.href =
        'agent-board-client:open?path=' + encodeURIComponent(url.pathname + url.search);
      return true;
    } catch {
      return false;
    }
  }

  for (const method of ['pushState', 'replaceState']) {
    const original = win.history[method];
    win.history[method] = function (...args) {
      if (swallowed(args[2])) {
        return undefined;
      }
      return original.apply(win.history, args);
    };
  }
});
