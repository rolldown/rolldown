module.exports = {
	description: 'deduplicates asset that have the same source',
	options: {
		input: ['main.js'],
		plugins: {
			buildStart() {
				this.emitFile({ type: 'asset', name: 'string.txt', source: 'string' });
				this.emitFile({ type: 'asset', name: 'stringSameSource.txt', source: 'string' });
				this.emitFile({
					type: 'asset',
					name: 'sameStringAsBuffer.txt',
					source: Buffer.from('string') // Test cross Buffer/string deduplication
				});
				// Different string source
				this.emitFile({ type: 'asset', name: 'otherString.txt', source: 'otherString' });

				const bufferSource = () => Buffer.from([0x62, 0x75, 0x66, 0x66, 0x65, 0x72]);
				this.emitFile({
					type: 'asset',
					name: 'buffer.txt',
					source: bufferSource()
				});
				this.emitFile({
					type: 'asset',
					name: 'bufferSameSource.txt',
					source: bufferSource()
				});
				this.emitFile({
					type: 'asset',
					name: 'sameBufferAsString.txt',
					source: bufferSource().toString() // Test cross Buffer/string deduplication
				});
				// Different buffer source
				this.emitFile({
					type: 'asset',
					name: 'otherBuffer.txt',
					source: Buffer.from('otherBuffer')
				});

				// specific file names will not be deduplicated
				this.emitFile({ type: 'asset', fileName: 'named/string.txt', source: 'named' });
				this.emitFile({
					type: 'asset',
					fileName: 'named/buffer.txt',
					source: bufferSource()
				});
				return null;
			}
		}
	}
};
