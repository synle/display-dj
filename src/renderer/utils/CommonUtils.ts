export function debounce(cb: (...args: any []) => any, delay = 300){
  let timerId : any;

  return (...inputs: any[]) => {
    clearTimeout(timerId);
    timerId = setTimeout(() => cb(...inputs), delay);
  }
}
