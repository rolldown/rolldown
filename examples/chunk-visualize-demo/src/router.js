// Simple router for the demo application

export class Router {
  constructor() {
    this.routes = new Map();
    this.currentRoute = '/';
  }

  register(path, handler) {
    this.routes.set(path, handler);
  }

  navigate(path) {
    this.currentRoute = path;
    const handler = this.routes.get(path);
    if (handler) {
      handler();
    } else {
      console.warn('Route not found:', path);
    }
  }

  getCurrentRoute() {
    return this.currentRoute;
  }
}
