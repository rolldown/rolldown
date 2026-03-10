import { createApp } from './app.js';
import { logger } from './utils/logger.js';
import { formatDate } from './utils/date-format.js';
import { validate } from './utils/validator.js';
import { getLocale } from './utils/locale.js';

const loadAdmin = () => import('./features/admin.js');
const loadDashboard = () => import('./features/dashboard.js');

createApp();
logger.info(formatDate(), validate(), getLocale());
export { loadAdmin, loadDashboard };
