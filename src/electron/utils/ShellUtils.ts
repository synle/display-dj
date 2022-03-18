import { spawn } from 'child_process';

export function executePowershell(shellToRun: string, delay = 100): Promise<string> {
  return new Promise((resolve, reject) => {
    setTimeout(() => {
      const child = spawn('powershell.exe', ['-Command', shellToRun]);

      let data = '';
      child.stdout.on('data', function (msg) {
        data += msg.toString();
      });

      child.on('exit', function (exitCode: string) {
        if (parseInt(exitCode) !== 0) {
          //Handle non-zero exit
          reject(exitCode);
        } else {
          resolve(data);
        }
      });
    }, delay);
  });
}
