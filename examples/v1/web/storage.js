const key = 'ubi.tasks.v1';
export const storage = {
  async get() { return localStorage.getItem(key); },
  async set(value) { localStorage.setItem(key, value); },
};
