import { logger } from './utils/logger.js';
import { validate } from './utils/validator.js';
import { formatDate } from './utils/date-format.js';
import { debounce } from './utils/helpers.js';

const loadDashboard = () => import('./features/dashboard.js');

logger.info(validate(), formatDate(), debounce(), loadDashboard);
