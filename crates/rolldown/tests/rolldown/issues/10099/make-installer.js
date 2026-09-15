// Mirrors element-plus/es/make-installer.mjs: returns a plain object.
export const makeInstaller = (components = []) => {
  const install = (app) => {
    components.forEach((c) => app.use(c));
  };
  return { version: '1.0.0', install };
};
