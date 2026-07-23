// Region-aware price display. Prices are authored in USD; we convert with fixed
// approximate rates and render the local symbol. Detection is best-effort from
// the browser's timezone/locale, and the user can override it.

export interface Currency {
	code: string;
	rate: number; // multiplier from USD
	round: number; // round the converted amount to the nearest this
}

export const CURRENCIES: Record<string, Currency> = {
	USD: { code: 'USD', rate: 1, round: 1 },
	BDT: { code: 'BDT', rate: 118, round: 10 },
	EUR: { code: 'EUR', rate: 0.92, round: 1 },
	GBP: { code: 'GBP', rate: 0.79, round: 1 },
	INR: { code: 'INR', rate: 83, round: 10 },
	AED: { code: 'AED', rate: 3.67, round: 1 },
	SAR: { code: 'SAR', rate: 3.75, round: 1 }
};

export const CURRENCY_CODES = Object.keys(CURRENCIES);

// Timezone → currency (checked in order). Covers the regions we call out.
const TZ_RULES: [RegExp, string][] = [
	[/Dhaka/i, 'BDT'],
	[/Kolkata|Calcutta/i, 'INR'],
	[/Dubai/i, 'AED'],
	[/Riyadh|Qatar|Bahrain|Kuwait|Muscat/i, 'SAR'],
	[/London/i, 'GBP'],
	[/^Europe\//i, 'EUR'],
	[/^America\//i, 'USD']
];

const REGION_RULES: Record<string, string> = {
	BD: 'BDT',
	IN: 'INR',
	AE: 'AED',
	SA: 'SAR',
	QA: 'SAR',
	GB: 'GBP',
	US: 'USD',
	DE: 'EUR',
	FR: 'EUR',
	ES: 'EUR',
	IT: 'EUR',
	NL: 'EUR'
};

/** Best-effort local currency from the browser. Browser-only. */
export function detectCurrency(): string {
	try {
		const tz = Intl.DateTimeFormat().resolvedOptions().timeZone || '';
		for (const [re, code] of TZ_RULES) if (re.test(tz)) return code;
		const region = (navigator.language.split('-')[1] || '').toUpperCase();
		if (REGION_RULES[region]) return REGION_RULES[region];
	} catch {
		/* SSR or blocked — fall through */
	}
	return 'USD';
}

/** Format a USD amount in the given currency, with its local symbol. */
export function formatPrice(usd: number, code: string): string {
	const c = CURRENCIES[code] ?? CURRENCIES.USD;
	const amount = Math.round((usd * c.rate) / c.round) * c.round;
	try {
		return new Intl.NumberFormat('en-US', {
			style: 'currency',
			currency: c.code,
			maximumFractionDigits: 0,
			currencyDisplay: 'narrowSymbol'
		}).format(amount);
	} catch {
		return `${amount} ${c.code}`;
	}
}
