// Reactive session: holds the tenant's API key, persisted to localStorage.
import { browser } from '$app/environment';

const STORAGE_KEY = 'ferrite.apiKey';

let apiKey = $state<string | null>(browser ? localStorage.getItem(STORAGE_KEY) : null);

export const session = {
	get apiKey() {
		return apiKey;
	},
	get isAuthed() {
		return !!apiKey;
	},
	set(key: string) {
		apiKey = key;
		if (browser) localStorage.setItem(STORAGE_KEY, key);
	},
	clear() {
		apiKey = null;
		if (browser) localStorage.removeItem(STORAGE_KEY);
	}
};
