// Small helpers for the bespoke SVG charts. No dependencies — the charts are
// hand-drawn so they inherit the design system's tokens and stay lean.

export interface Bucket {
	key: string;
	label: string;
	value: number;
}

function dayKey(d: Date): string {
	return `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;
}

/** Count how many of `dates` fall on each of the last `days` local days. */
export function bucketByDay(dates: (string | Date | null | undefined)[], days = 14): Bucket[] {
	const now = new Date();
	const buckets: Bucket[] = [];
	const index = new Map<string, number>();
	for (let i = days - 1; i >= 0; i--) {
		const d = new Date(now.getFullYear(), now.getMonth(), now.getDate() - i);
		index.set(dayKey(d), buckets.length);
		buckets.push({
			key: dayKey(d),
			label: d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' }),
			value: 0
		});
	}
	for (const raw of dates) {
		if (!raw) continue;
		const d = new Date(raw);
		if (Number.isNaN(d.getTime())) continue;
		const at = index.get(dayKey(d));
		if (at !== undefined) buckets[at].value += 1;
	}
	return buckets;
}

/** Sum a value across the last `days` days, bucketed by a date accessor. */
export function sumByDay<T>(
	items: T[],
	dateOf: (t: T) => string | Date | null | undefined,
	valueOf: (t: T) => number,
	days = 14
): Bucket[] {
	const now = new Date();
	const buckets: Bucket[] = [];
	const index = new Map<string, number>();
	for (let i = days - 1; i >= 0; i--) {
		const d = new Date(now.getFullYear(), now.getMonth(), now.getDate() - i);
		index.set(dayKey(d), buckets.length);
		buckets.push({
			key: dayKey(d),
			label: d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' }),
			value: 0
		});
	}
	for (const it of items) {
		const raw = dateOf(it);
		if (!raw) continue;
		const d = new Date(raw);
		if (Number.isNaN(d.getTime())) continue;
		const at = index.get(dayKey(d));
		if (at !== undefined) buckets[at].value += valueOf(it);
	}
	return buckets;
}

/** Running total — turns per-day counts into a cumulative curve. */
export function cumulative(values: number[]): number[] {
	let sum = 0;
	return values.map((v) => (sum += v));
}

/**
 * Build the `d` for a smooth line through points (Catmull-Rom → cubic Bézier).
 * `pts` are already in SVG coordinate space.
 */
export function smoothLine(pts: [number, number][]): string {
	if (pts.length === 0) return '';
	if (pts.length === 1) return `M ${pts[0][0]} ${pts[0][1]}`;
	const d = [`M ${pts[0][0]} ${pts[0][1]}`];
	for (let i = 0; i < pts.length - 1; i++) {
		const p0 = pts[i - 1] ?? pts[i];
		const p1 = pts[i];
		const p2 = pts[i + 1];
		const p3 = pts[i + 2] ?? p2;
		const c1x = p1[0] + (p2[0] - p0[0]) / 6;
		const c1y = p1[1] + (p2[1] - p0[1]) / 6;
		const c2x = p2[0] - (p3[0] - p1[0]) / 6;
		const c2y = p2[1] - (p3[1] - p1[1]) / 6;
		d.push(`C ${c1x} ${c1y} ${c2x} ${c2y} ${p2[0]} ${p2[1]}`);
	}
	return d.join(' ');
}
