declare module '@hensm/ddcci' {
  export function getMonitorList(): string[];
  export function getBrightness(id: string): Promise<number>;
  export function setBrightness(id: string, brightness: number): Promise<void>;
}
