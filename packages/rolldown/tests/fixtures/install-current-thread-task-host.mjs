export function installCurrentThreadTaskHost(binding) {
  if (binding.getRuntimeCapabilities().asyncRuntimeBuild !== true) {
    return () => {};
  }

  const contractVersion = binding.getCurrentThreadTaskHostContractVersion();
  if (contractVersion !== 2) {
    throw new TypeError(
      `Expected CurrentThread task-host contract version 2, received ${String(contractVersion)}`,
    );
  }

  const registration = binding.registerCurrentThreadTaskHost();
  if (
    registration === null ||
    typeof registration !== 'object' ||
    !Number.isInteger(registration.high) ||
    !Number.isInteger(registration.low) ||
    (registration.high === 0 && registration.low === 0)
  ) {
    throw new TypeError('The raw binding returned an invalid CurrentThread task-host registration');
  }

  return () => {
    binding.unregisterCurrentThreadTaskHost(registration.high, registration.low);
  };
}
