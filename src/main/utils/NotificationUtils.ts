import {Notification } from 'electron';

export function showNotification (body: string, title: string  = 'display-dj') {
  new Notification({ title, body}).show()
}
