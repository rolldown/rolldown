import { HMREventHandler } from './HotModuleReloadSetup.js';
import { time } from './time.js';

console.log('import.meta.hot', import.meta.hot);
if (import.meta.hot) {
  console.log('HMR enabled');
  import.meta.hot.accept(HMREventHandler);
}

class Draw {
  constructor(canvas) {
    this.timer = 0;
    this.canvas = canvas;
  }
  // Runs when the module is being swapped
  // Here we copy the state from the old module instance
  hotReload(oldModule) {
    this.timer = oldModule.timer;
  }

  draw() {
    const canvas = this.canvas;
    this.timer += time + 0;

    const ctx = canvas.getContext('2d');
    ctx.clearRect(0, 0, canvas.width, canvas.height);

    const rate = this.timer * 0.05;
    const radius = Math.min(canvas.width, canvas.height) * 0.5 * 0.9;

    const x = Math.cos(rate) * radius + canvas.width * 0.5;
    const y = Math.sin(rate) * radius + canvas.height * 0.5;

    ctx.beginPath();
    ctx.arc(x, y, 20, 0, Math.PI * 2);
    ctx.fillStyle = 'red';
    ctx.fill();
  }
}

export default Draw;
