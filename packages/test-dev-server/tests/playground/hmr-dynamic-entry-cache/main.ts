document.body.dataset.loaded = 'true';

(async () => {
  const pages = import.meta.glob('./page.ts');
  const page = (await pages['./page.ts']()) as { boot: () => void };
  page.boot();
})();
