import { Notification } from 'electron';
let lastNotification : Notification | undefined;

export function showNotification (body: string, title: string  = 'display-dj') {
  if(lastNotification){
    lastNotification.close();
    lastNotification = undefined;
  }
  lastNotification = new Notification({ title, body});
  lastNotification.show()

  setTimeout(() => {
    if(lastNotification){
      lastNotification.close();
      lastNotification = undefined;
    }
  }, 3000);
}