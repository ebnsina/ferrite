<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { Card, Icon } from '$lib/ui';
	import { Donut, Meter } from '$lib/charts';
	import { getUsage } from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import { humanizeError } from '$lib/humanize';
	import type { Usage } from '$lib/api/types';
	import { bytes } from '$lib/format';
	import { CreditCard, Timer, HardDrives } from 'phosphor-svelte';

	let usage = $state<Usage | null>(null);
	let error = $state<string | null>(null);
	let timer: ReturnType<typeof setInterval>;

	async function load() {
		try {
			usage = await getUsage();
			error = null;
		} catch (e) {
			error = humanizeError(e instanceof ApiError ? e.message : null, 'We couldn’t load your usage.');
		}
	}
	onMount(() => {
		load();
		timer = setInterval(load, 10000);
	});
	onDestroy(() => clearInterval(timer));

	const costSegments = $derived(
		usage
			? [
					{ label: 'Transcode', value: usage.cost.transcode, color: 'text-accent' },
					{ label: 'Storage', value: usage.cost.storage, color: 'text-success' }
				]
			: []
	);
</script>

<div class="mx-auto max-w-4xl">
	<div class="mb-8">
		<h1 class="flex items-center gap-2 text-2xl font-semibold tracking-tight">
			<span class="text-accent"><Icon icon={CreditCard} size={22} /></span> Usage
		</h1>
		<p class="mt-1 text-sm text-muted">Your consumption this month · estimated (billing is mocked).</p>
	</div>

	{#if error}
		<div class="mb-4 rounded-lg border border-danger/30 bg-danger/10 px-4 py-3 text-sm text-danger">{error}</div>
	{/if}

	{#if usage}
		{@const u = usage}
		<!-- Cost hero -->
		<Card class="mb-6">
			<div class="flex flex-wrap items-center gap-8">
				<Donut segments={costSegments} size={150} thickness={16}>
					{#snippet center()}
						<span class="mono text-2xl font-semibold text-accent">${u.cost.total.toFixed(0)}</span>
						<span class="text-[10px] text-muted">est. cost</span>
					{/snippet}
				</Donut>
				<div class="flex-1">
					<p class="text-xs text-muted">Estimated cost this month</p>
					<p class="mono mt-1 text-4xl font-semibold text-accent">${u.cost.total.toFixed(2)}</p>
					<div class="mt-4 flex flex-col gap-2 text-sm">
						<div class="flex items-center gap-2">
							<span class="h-2.5 w-2.5 rounded-full bg-accent"></span>
							<span class="text-muted">Transcode</span>
							<span class="mono ml-auto font-medium">${u.cost.transcode.toFixed(2)}</span>
						</div>
						<div class="flex items-center gap-2">
							<span class="h-2.5 w-2.5 rounded-full bg-success"></span>
							<span class="text-muted">Storage</span>
							<span class="mono ml-auto font-medium">${u.cost.storage.toFixed(2)}</span>
						</div>
					</div>
				</div>
			</div>
		</Card>

		<!-- Consumption meters -->
		<div class="grid gap-6 sm:grid-cols-2">
			<Card>
				<div class="mb-4 flex items-center gap-2">
					<span class="text-muted"><Icon icon={Timer} size={16} /></span>
					<h2 class="text-sm font-medium">Transcoding</h2>
				</div>
				<p class="mono mb-4 text-3xl font-semibold">{u.minutes.toFixed(1)}<span class="ml-1 text-base font-normal text-muted">min</span></p>
				<Meter label="This month" value={`${u.minutes.toFixed(1)} min`} fraction={u.minutes / 2000} hint="of a 2,000-min reference tier" />
			</Card>

			<Card>
				<div class="mb-4 flex items-center gap-2">
					<span class="text-muted"><Icon icon={HardDrives} size={16} /></span>
					<h2 class="text-sm font-medium">Storage</h2>
				</div>
				<p class="mono mb-4 text-3xl font-semibold">{bytes(u.storage_bytes)}</p>
				<Meter label="Stored" value={bytes(u.storage_bytes)} fraction={u.storage_gb / 100} color="text-success" hint="of a 100 GB reference tier" />
			</Card>
		</div>

		<p class="mt-6 text-center text-xs text-muted">
			Reference tiers are illustrative — real billing isn’t enabled yet.
		</p>
	{:else if !error}
		<Card><p class="py-10 text-center text-sm text-muted">Loading…</p></Card>
	{/if}
</div>
