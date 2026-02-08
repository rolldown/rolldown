// Main entry point for chunk visualization demo
import { createApp } from './app.js';
import { logger } from './utils/logger.js';
import './styles.css';

logger.info('Application starting...');

// Dynamically import admin module when needed
const loadAdmin = () => import('./features/admin.js');

// Dynamically import dashboard when needed
const loadDashboard = () => import('./features/dashboard.js');

async function bootstrap() {
  const app = createApp();

  logger.info('App initialized', app);

  // Example of dynamic imports based on conditions
  if (window.location.hash === '#admin') {
    const admin = await loadAdmin();
    admin.initialize();
  } else if (window.location.hash === '#dashboard') {
    const dashboard = await loadDashboard();
    dashboard.render();
  }

  return app;
}

bootstrap().catch((err) => {
  logger.error('Failed to bootstrap application:', err);
});

export { loadAdmin, loadDashboard };
