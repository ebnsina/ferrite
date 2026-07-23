// Reactive theme, applied to <html data-theme> and persisted. Initial value is
// set pre-paint by the inline script in app.html; this mirrors and mutates it.
import { browser } from '$app/environment';

function current(): 'dark' | 'light' {
	if (!browser) return 'dark';
	return document.documentElement.getAttribute('data-theme') === 'light' ? 'light' : 'dark';
}

let mode = $state<'dark' | 'light'>(current());

export const theme = {
	get mode() {
		return mode;
	},
	set(next: 'dark' | 'light') {
		mode = next;
		if (browser) {
			document.documentElement.setAttribute('data-theme', next);
			localStorage.setItem('ferrite.theme', next);
		}
	},
	toggle() {
		this.set(mode === 'dark' ? 'light' : 'dark');
	}
};
