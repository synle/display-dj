declare const ipcRenderer: Electron.IpcRenderer;
declare const origFetch: ((input: RequestInfo, init?: RequestInit | undefined) => Promise<Response>) & typeof fetch;
declare function _fetch<T>(input: RequestInfo, initOptions?: RequestInit): Promise<T>;
