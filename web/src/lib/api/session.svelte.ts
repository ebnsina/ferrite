// Reactive session: the dashboard user's JWT + identity, persisted to localStorage.
import { browser } from '$app/environment';

const STORAGE_KEY = 'ferrite.session';

export interface SessionData {
	token: string;
	user: { id: string; email: string; role: string };
	tenant: { id: string; name: string };
}

function load(): SessionData | null {
	if (!browser) return null;
	try {
		const raw = localStorage.getItem(STORAGE_KEY);
		return raw ? (JSON.parse(raw) as SessionData) : null;
	} catch {
		return null;
	}
}

let data = $state<SessionData | null>(load());

export const session = {
	get token() {
		return data?.token ?? null;
	},
	get user() {
		return data?.user ?? null;
	},
	get tenant() {
		return data?.tenant ?? null;
	},
	get isAuthed() {
		return !!data;
	},
	set(next: SessionData) {
		data = next;
		if (browser) localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
	},
	clear() {
		data = null;
		if (browser) localStorage.removeItem(STORAGE_KEY);
	}
};
