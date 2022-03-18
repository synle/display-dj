declare module '@hensm/ddcci' {
  export function getMonitorList(): string[];
  export function getBrightness(id: string): Promise<number>;
  export function setBrightness(id: string, brightness: number): Promise<void>;
}

// for the UI
declare module "*.svg" {
  const content: any;
  export default content;
}

declare module '*.jpg' {
  const content: string;
  export default content;
}

declare module '*.png' {
  const content: string;
  export default content;
}

declare module '*.json' {
  const content: string;
  export default content;
}
