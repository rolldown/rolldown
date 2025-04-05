import HotModuleReloadSetup from './HotModuleReloadSetup.js';

async function init() {
  // Setup the canvas & render loop
  const canvas = document.createElement('canvas');
  document.body.appendChild(canvas);

  function resizeCanvas() {
    canvas.width = window.innerWidth;
    canvas.height = window.innerHeight;
  }
  resizeCanvas();
  window.addEventListener('resize', resizeCanvas);

  // Setup HMR
  const hmr = new HotModuleReloadSetup();
  // Load a module that will be updated dynamically
  hmr.import(await import('./Draw.js'), canvas);
  // Now we access it through hmr.instances['Draw']
  // which will point to the new module when it gets swapped
  function draw() {
    hmr.instances['Draw'].draw();
    requestAnimationFrame(draw);
  }
  draw();
}

init();
