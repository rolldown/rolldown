export function installCurrentThreadTaskHost(binding) {
  if (binding.getRuntimeCapabilities().asyncRuntimeBuild !== true) {
    return () => {};
  }

  const contractVersion = binding.getCurrentThreadTaskHostContractVersion();
  if (contractVersion !== 4) {
    throw new TypeError(
      `Expected CurrentThread task-host contract version 4, received ${String(contractVersion)}`,
    );
  }

  const registration = binding.reserveCurrentThreadHostRegistration();
  if (
    registration === null ||
    typeof registration !== 'object' ||
    !Number.isInteger(registration.high) ||
    !Number.isInteger(registration.low) ||
    (registration.high === 0 && registration.low === 0)
  ) {
    throw new TypeError('The raw binding returned an invalid CurrentThread task-host reservation');
  }

  try {
    binding.registerCurrentThreadTaskHost(registration.high, registration.low);
  } catch (error) {
    binding.unregisterCurrentThreadTaskHost(registration.high, registration.low);
    throw error;
  }

  return () => {
    binding.unregisterCurrentThreadTaskHost(registration.high, registration.low);
  };
}
