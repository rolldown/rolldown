

import shell from 'shelljs';


/**
 * 
 * @param {string} command 
 * @param {string} dir 
 * @param {import('shelljs').ExecOptions} userOptions 
 * @returns {Promise<string>}
 */
export async function runCommand(command, dir, userOptions) {
	return new Promise((resolve, reject) => {
		try {
			shell.exec(
				command,
				{
					cwd: dir,
					encoding: 'GBK',
					async: true,
					silent: userOptions?.silent === undefined ? true : userOptions?.silent,
					...userOptions
				},
				(code, output, err) => {
					if (code === 0) {
						resolve('success');
					} else if (err) {
						reject(err.toString());
					}

					const outputStr = output.toString();
					if (outputStr && !(userOptions?.silent ?? true)) {
						console.log(outputStr);
					}
				}
			);
		} catch (e) {
			reject(e);
		}
	});
}

const redText = '\u001b[31m';
const greenText = '\u001b[32m';
const resetText = '\u001b[0m';
/**
 * 
 * @param {string} text 
 * @returns {string}
 */
export const errrorText = (text) => {
	return redText + text + resetText
}
/**
 * 
 * @param {string} text 
 * @returns {string}
 */
export const successText = (text) => {
	return greenText + text + resetText
}