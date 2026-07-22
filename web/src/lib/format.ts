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
