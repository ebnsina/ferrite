<script lang="ts">
	// A 270° radial gauge for a single 0–100 value (e.g. success rate).
	interface Props {
		value: number; // 0–100
		size?: number;
		thickness?: number;
		color?: string; // Tailwind text-color class
		label?: string;
	}

	let { value, size = 160, thickness = 14, color = 'text-accent', label }: Props = $props();

	const SWEEP = 270; // degrees of arc
	const r = $derived((size - thickness) / 2);
	const c = $derived(2 * Math.PI * r);
	const arcLen = $derived((SWEEP / 360) * c);
	const pct = $derived(Math.max(0, Math.min(100, value)));
	const filled = $derived((pct / 100) * arcLen);
	// Rotate so the gap sits at the bottom (centered): start at 135°.
	const rot = $derived(135);
</script>

<div class="relative flex items-center justify-center" style={`width:${size}px;height:${size}px`}>
	<svg width={size} height={size} viewBox={`0 0 ${size} ${size}`}>
		<circle
			cx={size / 2}
			cy={size / 2}
			{r}
			fill="none"
			stroke="currentColor"
			stroke-width={thickness}
			stroke-linecap="round"
			stroke-dasharray={`${arcLen} ${c}`}
			transform={`rotate(${rot} ${size / 2} ${size / 2})`}
			class="text-surface-2"
		/>
		<circle
			cx={size / 2}
			cy={size / 2}
			{r}
			fill="none"
			stroke="currentColor"
			stroke-width={thickness}
			stroke-linecap="round"
			stroke-dasharray={`${filled} ${c}`}
			transform={`rotate(${rot} ${size / 2} ${size / 2})`}
			class={color}
			style="transition: stroke-dasharray 0.6s ease"
		/>
	</svg>
	<div class="absolute flex flex-col items-center">
		<span class="mono text-3xl font-semibold">{Math.round(pct)}<span class="text-lg text-muted">%</span></span>
		{#if label}<span class="mt-0.5 text-xs text-muted">{label}</span>{/if}
	</div>
</div>
