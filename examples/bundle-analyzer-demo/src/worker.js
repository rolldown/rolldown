import { logger } from './utils/logger.js';
import { formatDate, formatTime, debounce } from './utils/helpers.js';

const loadDashboard = () => import('./features/dashboard.js');

logger.info(formatDate(), formatTime(), debounce(), loadDashboard);
