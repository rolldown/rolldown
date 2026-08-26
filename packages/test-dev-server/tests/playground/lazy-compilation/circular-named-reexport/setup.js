const status = document.getElementById('circular-named-reexport-status');

status.textContent = 'ready';

document.getElementById('circular-named-reexport-btn').addEventListener('click', async () => {
  status.textContent = 'loading';
  try {
    const { exampleResult, publicValue } = await import('./entry.js');
    status.textContent = `${exampleResult} ${publicValue()}`;
  } catch (error) {
    status.textContent = `error: ${error.message}`;
  }
});
