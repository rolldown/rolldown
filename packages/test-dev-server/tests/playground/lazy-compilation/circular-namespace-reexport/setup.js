const status = document.getElementById('circular-namespace-reexport-status');

status.textContent = 'ready';

document.getElementById('circular-namespace-reexport-btn').addEventListener('click', async () => {
  status.textContent = 'loading';
  try {
    const { exampleResult, ns } = await import('./entry.js');
    status.textContent = `${exampleResult} ${ns.value()}`;
  } catch (error) {
    status.textContent = `error: ${error.message}`;
  }
});
