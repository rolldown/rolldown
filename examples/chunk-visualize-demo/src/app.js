import { Header } from './components/Header.js';
import { Footer } from './components/Footer.js';
import { Router } from './router.js';

export function createApp() {
  const app = {
    name: 'Demo Application',
    version: '1.0.0',
    components: {
      header: new Header(),
      footer: new Footer(),
    },
    router: new Router(),
  };

  app.render = function () {
    console.log('Rendering application...');
    this.components.header.render();
    this.components.footer.render();
  };

  return app;
}
