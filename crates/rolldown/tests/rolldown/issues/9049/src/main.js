import { svc0 } from './services/svc0.js';
import { svc1 } from './services/svc1.js';

// Side effect to prevent tree-shaking
document.title = [svc0('i'), svc1('i')].join('');

const routes = {};
routes['r0'] = () => import('./routes/route0.js');
routes['r1'] = () => import('./routes/route1.js');

const routeName = location.hash.slice(1) || 'r0';
if (routes[routeName]) {
  routes[routeName]().then((m) => {
    document.body.textContent = m.render();
  });
}
