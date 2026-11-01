const routes = {
  page: () => import('./page.js'),
  form: () => import('./form.js'),
};
globalThis.__11397r_done = routes.form().then((form) => {
  globalThis.__11397r_formNs = form;
});
