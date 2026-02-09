import { createApp } from './app.js';
import { logger } from './utils/logger.js';
import './styles.css';

const loadAdmin = () => import('./features/admin.js');
const loadDashboard = () => import('./features/dashboard.js');

createApp();
logger.info();

export { loadAdmin, loadDashboard };
