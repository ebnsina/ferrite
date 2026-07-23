<script lang="ts">
	import { smoothLine } from './util';

	interface Props {
		values: number[];
		color?: string; // Tailwind text-color class
		height?: number;
	}

	let { values, color = 'text-accent', height = 40 }: Props = $props();

	const W = 200;
	const H = 60;
	const pad = 4;
	const uid = Math.random().toString(36).slice(2, 8);

	const max = $derived(Math.max(1, ...values));
	const min = $derived(Math.min(0, ...values));
	const n = $derived(values.length);
	const x = (i: number) => (n <= 1 ? W / 2 : pad + (i / (n - 1)) * (W - 2 * pad));
	const y = (v: number) => pad + (1 - (v - min) / (max - min || 1)) * (H - 2 * pad);

	const line = $derived(smoothLine(values.map((v, i) => [x(i), y(v)])));
	const areaD = $derived(n ? `${line} L ${x(n - 1)} ${H} L ${x(0)} ${H} Z` : '');
</script>

<svg
	viewBox={`0 0 ${W} ${H}`}
	width="100%"
	{height}
	preserveAspectRatio="none"
	class={`block ${color}`}
>
	<defs>
		<linearGradient id={`sp-${uid}`} x1="0" y1="0" x2="0" y2="1">
			<stop offset="0%" stop-color="currentColor" stop-opacity="0.25" />
			<stop offset="100%" stop-color="currentColor" stop-opacity="0" />
		</linearGradient>
	</defs>
	{#if n > 0}
		<path d={areaD} fill={`url(#sp-${uid})`} />
		<path
			d={line}
			fill="none"
			stroke="currentColor"
			stroke-width="2"
			stroke-linecap="round"
			stroke-linejoin="round"
			vector-effect="non-scaling-stroke"
		/>
	{/if}
</svg>
