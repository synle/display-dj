import { spawn, exec } from 'child_process';

export function executePowershell(shellToRun: string, delay = 50): Promise<string> {
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

export function executeBash(shellToRun: string, delay = 50): Promise<string> {
  return new Promise((resolve, reject) => {
    setTimeout(() => {
      exec(shellToRun, (error, stdout, stderr) => {
        if (error) {
          return reject(stderr);
        }

        resolve(stdout);
      });
    }, delay);
  });
}

export function executeBashFork(shellToRun: string, delay = 50): Promise<string> {
  return new Promise((resolve, reject) => {
    setTimeout(() => {
      const [tool, ...rest] = shellToRun.split(' ');

      const command = spawn(shellToRun, [rest.join(' ')]);

      let stderr = '';
      let stdout = '';

      command.stdout.on('data', (data) => {
        stdout += data.toString();
      });

      command.stderr.on('data', (data) => {
        stdout += data.toString();
      });

      command.on('error', function (err) {
        console.error(err);
        reject(stderr);
      });

      command.on('exit', function (exitCode: string) {
        if (parseInt(exitCode) !== 0) {
          reject(stderr); //Handle non-zero exit
        } else {
          resolve(stdout);
        }
      });
    }, delay);
  });
}
