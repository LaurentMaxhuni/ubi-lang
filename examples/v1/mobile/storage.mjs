import { Preferences } from '@capacitor/preferences';
const key = 'ubi.tasks.v1';
export const storage = {
  async get() { return (await Preferences.get({ key })).value; },
  async set(value) { await Preferences.set({ key, value }); },
};
