(() => {
  window.addEventListener('click', (event) => {
    if (
      event.defaultPrevented ||
      event.button !== 0 ||
      event.metaKey ||
      event.ctrlKey ||
      event.shiftKey ||
      event.altKey
    ) {
      return;
    }

    if (!(event.target instanceof Element)) {
      return;
    }

    const anchor = event.target.closest('a[href]');
    if (!anchor || anchor.hasAttribute('download')) {
      return;
    }

    let url;
    try {
      url = new URL(anchor.href);
    } catch {
      return;
    }

    const isWeb = url.protocol === 'http:' || url.protocol === 'https:';
    if (isWeb && url.origin !== location.origin) {
      event.preventDefault();
      window.open(url.href, '_blank', 'noopener');
    }
  });
})();
