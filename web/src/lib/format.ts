// Formatting via the Intl web API (locale-aware, no custom logic).

const BYTE_UNITS = ['byte', 'kilobyte', 'megabyte', 'gigabyte', 'terabyte'] as const;

export function bytes(n: number | null | undefined): string {
	if (!n) return '—';
	let i = 0;
	let v = n;
	while (v >= 1024 && i < BYTE_UNITS.length - 1) {
		v /= 1024;
		i++;
	}
	return new Intl.NumberFormat(undefined, {
		style: 'unit',
		unit: BYTE_UNITS[i],
		unitDisplay: 'short',
		maximumFractionDigits: 1
	}).format(v);
}

const rtf = new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' });

export function timeAgo(iso: string): string {
	const secs = Math.round((new Date(iso).getTime() - Date.now()) / 1000);
	const abs = Math.abs(secs);
	if (abs < 60) return rtf.format(secs, 'second');
	if (abs < 3600) return rtf.format(Math.round(secs / 60), 'minute');
	if (abs < 86400) return rtf.format(Math.round(secs / 3600), 'hour');
	return rtf.format(Math.round(secs / 86400), 'day');
}

/** Time-of-day salutation. */
export function greeting(d = new Date()): string {
	const h = d.getHours();
	if (h < 12) return 'Good morning';
	if (h < 18) return 'Good afternoon';
	return 'Good evening';
}

/** A friendly first name from an email local part ("maya.chen@x" → "Maya"). */
export function nameFromEmail(email: string | undefined | null): string {
	const local = (email ?? '').split('@')[0].split(/[.\-_+]/)[0];
	return local ? local.charAt(0).toUpperCase() + local.slice(1) : 'there';
}

/** e.g. "Thursday, July 23". */
export function longDate(d = new Date()): string {
	return new Intl.DateTimeFormat(undefined, {
		weekday: 'long',
		month: 'long',
		day: 'numeric'
	}).format(d);
}
