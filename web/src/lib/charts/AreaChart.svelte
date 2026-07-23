<script lang="ts">
	import { smoothLine } from './util';

	interface Series {
		name: string;
		color: string; // a Tailwind text-color class, e.g. 'text-accent'
		values: number[];
	}

	interface Props {
		series: Series[];
		labels: string[];
		height?: number;
		/** Format a value for the tooltip (e.g. bytes). */
		format?: (n: number) => string;
		/** Fill the area under the line (first-glance density). */
		area?: boolean;
	}

	let { series, labels, height = 200, format = (n) => `${n}`, area = true }: Props = $props();

	// Fixed coordinate space; the SVG stretches to fit (preserveAspectRatio none)
	// while strokes stay crisp via vector-effect. Points sit at column centres so
	// the hover overlay lines up exactly.
	const W = 1000;
	const H = 320;
	const padY = 18;

	const n = $derived(labels.length);
	const max = $derived(Math.max(1, ...series.flatMap((s) => s.values)));
	const uid = Math.random().toString(36).slice(2, 8);

	const x = (i: number) => ((i + 0.5) / Math.max(1, n)) * W;
	const y = (v: number) => padY + (1 - v / max) * (H - 2 * padY);

	function linePath(values: number[]): string {
		return smoothLine(values.map((v, i) => [x(i), y(v)]));
	}
	function areaPath(values: number[]): string {
		if (values.length === 0) return '';
		const line = linePath(values);
		return `${line} L ${x(values.length - 1)} ${H} L ${x(0)} ${H} Z`;
	}

	let hover = $state(-1);
</script>

<div class="relative w-full" style={`height:${height}px`}>
	<svg
		viewBox={`0 0 ${W} ${H}`}
		width="100%"
		height="100%"
		preserveAspectRatio="none"
		class="overflow-visible"
	>
		<defs>
			{#each series as s, i (s.name)}
				<linearGradient id={`ac-${uid}-${i}`} x1="0" y1="0" x2="0" y2="1">
					<stop offset="0%" stop-color="currentColor" stop-opacity="0.28" class={s.color} />
					<stop offset="100%" stop-color="currentColor" stop-opacity="0" class={s.color} />
				</linearGradient>
			{/each}
		</defs>

		<!-- horizontal gridlines -->
		{#each [0, 0.5, 1] as g (g)}
			<line
				x1="0"
				x2={W}
				y1={padY + g * (H - 2 * padY)}
				y2={padY + g * (H - 2 * padY)}
				class="text-border"
				stroke="currentColor"
				stroke-width="1"
				vector-effect="non-scaling-stroke"
				stroke-dasharray="3 5"
				opacity="0.6"
			/>
		{/each}

		{#each series as s, i (s.name)}
			{#if area}
				<path d={areaPath(s.values)} fill={`url(#ac-${uid}-${i})`} class={s.color} />
			{/if}
			<path
				d={linePath(s.values)}
				fill="none"
				stroke="currentColor"
				stroke-width="2"
				stroke-linecap="round"
				stroke-linejoin="round"
				vector-effect="non-scaling-stroke"
				class={s.color}
			/>
		{/each}

		{#if hover >= 0}
			<line
				x1={x(hover)}
				x2={x(hover)}
				y1={padY}
				y2={H - padY}
				class="text-muted"
				stroke="currentColor"
				stroke-width="1"
				vector-effect="non-scaling-stroke"
				opacity="0.5"
			/>
			{#each series as s, i (s.name)}
				<circle cx={x(hover)} cy={y(s.values[hover] ?? 0)} r="5" class={`${s.color} fill-current`} />
				<circle
					cx={x(hover)}
					cy={y(s.values[hover] ?? 0)}
					r="5"
					fill="none"
					stroke="var(--surface)"
					stroke-width="2"
					vector-effect="non-scaling-stroke"
				/>
			{/each}
		{/if}
	</svg>

	<!-- hover columns (one per point) -->
	<div class="absolute inset-0 flex" role="presentation" onpointerleave={() => (hover = -1)}>
		{#each labels as label, i (i)}
			<button
				type="button"
				class="h-full flex-1"
				aria-label={label}
				onpointerenter={() => (hover = i)}
				onfocus={() => (hover = i)}
			></button>
		{/each}
	</div>

	{#if hover >= 0}
		<div
			class="pointer-events-none absolute top-2 z-10 -translate-x-1/2 rounded-lg border border-border bg-surface/95 px-3 py-2 text-xs shadow-lg backdrop-blur"
			style={`left:clamp(64px, ${((hover + 0.5) / Math.max(1, n)) * 100}%, calc(100% - 64px))`}
		>
			<p class="mb-1 font-medium text-muted">{labels[hover]}</p>
			{#each series as s (s.name)}
				<div class="flex items-center gap-2 whitespace-nowrap">
					<span class={`inline-block h-2 w-2 rounded-full ${s.color} bg-current`}></span>
					<span class="text-muted">{s.name}</span>
					<span class="mono ml-auto font-semibold text-fg">{format(s.values[hover] ?? 0)}</span>
				</div>
			{/each}
		</div>
	{/if}
</div>

{#if labels.length > 1}
	<div class="mt-2 flex justify-between text-[10px] text-muted">
		<span>{labels[0]}</span>
		<span>{labels[Math.floor((labels.length - 1) / 2)]}</span>
		<span>{labels[labels.length - 1]}</span>
	</div>
{/if}
