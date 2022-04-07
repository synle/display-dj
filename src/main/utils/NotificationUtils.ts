import { Notification } from 'electron';
import { debounce } from 'src/renderer/utils/CommonUtils';

let lastNotification : Notification | undefined;

export function dismissLastNotification(){
  if(lastNotification){
    lastNotification.close();
    lastNotification = undefined;
  }
}

export function showNotification (body: string, title: string  = 'display-dj') {
  dismissLastNotification();

  lastNotification = new Notification({ title, body});
  lastNotification.show()

  debouncedDismissNotification()
}

const debouncedDismissNotification = debounce(dismissLastNotification, 3000);
