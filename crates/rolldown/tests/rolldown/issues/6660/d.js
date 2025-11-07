export async function loadEventStreamCapability() {
  const { EventStreamSerde } = await import('./e.js');
  console.log(`EventStreamSerde: `, EventStreamSerde);
}

loadEventStreamCapability();
