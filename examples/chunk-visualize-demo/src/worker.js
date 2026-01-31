// Worker entry point - shares common modules with main.js
import { logger } from './utils/logger.js';
import { formatDate, formatTime, debounce } from './utils/helpers.js';

logger.info('Worker starting...');

// Dynamically import dashboard (shared with main.js)
const loadDashboard = () => import('./features/dashboard.js');

class WorkerProcess {
  constructor(name) {
    this.name = name;
    this.startTime = new Date();
    this.tasks = [];
  }

  logStatus() {
    logger.info(
      `Worker "${this.name}" status:`,
      `Started: ${formatDate(this.startTime)} ${formatTime(this.startTime)}`,
      `Tasks: ${this.tasks.length}`,
    );
  }

  addTask(task) {
    this.tasks.push(task);
    logger.info(`Task added: ${task.name}`);
  }

  async processTasks() {
    // Use dashboard for rendering task progress
    const dashboard = await loadDashboard();
    dashboard.render();

    for (const task of this.tasks) {
      logger.info(`Processing task: ${task.name}`);
    }
  }
}

const worker = new WorkerProcess('background-worker');

// Debounced status logging
const debouncedStatus = debounce(() => worker.logStatus(), 1000);

worker.logStatus();
debouncedStatus();

export { WorkerProcess, worker };
