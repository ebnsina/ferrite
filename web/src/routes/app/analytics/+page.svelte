<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { Card, Icon } from '$lib/ui';
	import { AreaChart, Donut, Gauge, bucketByDay, sumByDay, cumulative } from '$lib/charts';
	import { listAssets, listJobs, getUsage } from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import { humanizeError } from '$lib/humanize';
	import type { Asset, Job, JobState, Usage } from '$lib/api/types';
	import { bytes } from '$lib/format';
	import { ChartLineUp, ArrowsClockwise, Timer } from 'phosphor-svelte';

	let assets = $state<Asset[]>([]);
	let jobs = $state<Job[]>([]);
	let usage = $state<Usage | null>(null);
	let error = $state<string | null>(null);
	let range = $state(14);
	let timer: ReturnType<typeof setInterval>;

	const ACTIVE: JobState[] = ['queued', 'probing', 'transcoding', 'packaging', 'uploading'];

	async function load() {
		try {
			[assets, jobs, usage] = await Promise.all([listAssets(), listJobs(), getUsage()]);
			error = null;
		} catch (e) {
			error = humanizeError(e instanceof ApiError ? e.message : null, 'We couldn’t load your analytics.');
		}
	}
	onMount(() => {
		load();
		timer = setInterval(load, 5000);
	});
	onDestroy(() => clearInterval(timer));

	const inflight = $derived(jobs.filter((j) => ACTIVE.includes(j.state)));
	const completed = $derived(jobs.filter((j) => j.state === 'completed'));
	const failed = $derived(jobs.filter((j) => j.state === 'failed'));
	const successRate = $derived(
		completed.length + failed.length === 0 ? 0 : (completed.length / (completed.length + failed.length)) * 100
	);
	const storageBytes = $derived(usage?.storage_bytes ?? assets.reduce((s, a) => s + (a.bytes ?? 0), 0));

	const avgProcSecs = $derived.by(() => {
		const done = completed.filter((j) => j.finished_at);
		if (!done.length) return null;
		const t = done.reduce(
			(s, j) => s + (new Date(j.finished_at!).getTime() - new Date(j.queued_at).getTime()),
			0
		);
		return t / done.length / 1000;
	});

	const labels = $derived(bucketByDay([], range).map((b) => b.label));
	const completedByDay = $derived(bucketByDay(completed.map((j) => j.finished_at ?? j.queued_at), range).map((b) => b.value));
	const failedByDay = $derived(bucketByDay(failed.map((j) => j.finished_at ?? j.queued_at), range).map((b) => b.value));

	const storageSeries = $derived.by(() => {
		const start = new Date();
		start.setDate(start.getDate() - (range - 1));
		start.setHours(0, 0, 0, 0);
		const base = assets.filter((a) => new Date(a.created_at) < start).reduce((s, a) => s + (a.bytes ?? 0), 0);
		const perDay = sumByDay(assets, (a) => a.created_at, (a) => a.bytes ?? 0, range).map((b) => b.value);
		return cumulative(perDay).map((v) => v + base);
	});

	const stateSegments = $derived([
		{ label: 'Completed', value: completed.length, color: 'text-success' },
		{ label: 'In progress', value: inflight.length, color: 'text-accent' },
		{ label: 'Failed', value: failed.length, color: 'text-danger' }
	]);

	const assetStatus = $derived([
		{ label: 'Ready', value: assets.filter((a) => a.status === 'ready').length, color: 'text-success' },
		{ label: 'Processing', value: assets.filter((a) => a.status === 'processing').length, color: 'text-accent' },
		{ label: 'Uploading', value: assets.filter((a) => a.status === 'uploading').length, color: 'text-warning' },
		{ label: 'Error', value: assets.filter((a) => a.status === 'error').length, color: 'text-danger' }
	]);

	function fmtDuration(secs: number): string {
		const s = Math.round(secs);
		if (s < 60) return `${s}s`;
		const m = Math.floor(s / 60);
		if (m < 60) return `${m}m ${s % 60}s`;
		return `${Math.floor(m / 60)}h ${m % 60}m`;
	}

	const summary = $derived([
		{ label: 'Total jobs', value: String(jobs.length) },
		{ label: 'Success rate', value: completed.length + failed.length === 0 ? '—' : `${successRate.toFixed(0)}%` },
		{ label: 'Avg processing', value: avgProcSecs === null ? '—' : fmtDuration(avgProcSecs) },
		{ label: 'Storage used', value: bytes(storageBytes) }
	]);
</script>

<div class="mx-auto max-w-6xl">
	<div class="mb-8 flex flex-wrap items-center justify-between gap-4">
		<div>
			<h1 class="flex items-center gap-2 text-2xl font-semibold tracking-tight">
				<span class="text-accent"><Icon icon={ChartLineUp} size={22} /></span> Analytics
			</h1>
			<p class="mt-1 text-sm text-muted">How your workspace is performing.</p>
		</div>
		<div class="flex items-center gap-3">
			<span class="flex items-center gap-1.5 text-xs text-muted">
				<Icon icon={ArrowsClockwise} size={13} /> auto-refresh
			</span>
			<div class="flex items-center gap-1 rounded-lg border border-border bg-surface-2 p-0.5 text-xs">
				{#each [14, 30] as r (r)}
					<button
						onclick={() => (range = r)}
						class={`rounded-md px-2.5 py-1 transition-colors ${range === r ? 'bg-surface text-fg shadow-sm' : 'text-muted hover:text-fg'}`}
					>{r}d</button>
				{/each}
			</div>
		</div>
	</div>

	{#if error}
		<div class="mb-4 rounded-lg border border-danger/30 bg-danger/10 px-4 py-3 text-sm text-danger">{error}</div>
	{/if}

	<!-- Summary -->
	<div class="mb-6 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
		{#each summary as s (s.label)}
			<Card>
				<p class="text-xs text-muted">{s.label}</p>
				<p class="mono mt-1 text-2xl font-semibold">{s.value}</p>
			</Card>
		{/each}
	</div>

	<!-- Processing activity + success gauge -->
	<div class="mb-6 grid gap-6 lg:grid-cols-3">
		<Card class="lg:col-span-2">
			<div class="mb-4">
				<h2 class="text-sm font-medium">Processing activity</h2>
				<p class="text-xs text-muted">Completed vs failed per day</p>
			</div>
			<AreaChart
				{labels}
				series={[
					{ name: 'Completed', color: 'text-success', values: completedByDay },
					{ name: 'Failed', color: 'text-danger', values: failedByDay }
				]}
				height={220}
			/>
			<div class="mt-3 flex gap-5 text-xs">
				<span class="flex items-center gap-1.5"><span class="h-2 w-2 rounded-full bg-success"></span> <span class="text-muted">Completed</span></span>
				<span class="flex items-center gap-1.5"><span class="h-2 w-2 rounded-full bg-danger"></span> <span class="text-muted">Failed</span></span>
			</div>
		</Card>

		<Card>
			<h2 class="mb-1 text-sm font-medium">Success rate</h2>
			<p class="text-xs text-muted">Completed of finished jobs</p>
			<div class="my-4 flex justify-center">
				<Gauge value={successRate} color={successRate >= 90 ? 'text-success' : successRate >= 60 ? 'text-warning' : 'text-danger'} label="success" />
			</div>
			<div class="grid grid-cols-3 gap-2 border-t border-border pt-4 text-center">
				<div><p class="mono text-lg font-semibold text-success">{completed.length}</p><p class="text-[10px] text-muted">Completed</p></div>
				<div><p class="mono text-lg font-semibold text-accent">{inflight.length}</p><p class="text-[10px] text-muted">In progress</p></div>
				<div><p class="mono text-lg font-semibold text-danger">{failed.length}</p><p class="text-[10px] text-muted">Failed</p></div>
			</div>
		</Card>
	</div>

	<!-- Storage growth + job states -->
	<div class="mb-6 grid gap-6 lg:grid-cols-3">
		<Card class="lg:col-span-2">
			<div class="mb-4">
				<h2 class="text-sm font-medium">Storage growth</h2>
				<p class="text-xs text-muted">Cumulative source footage over {range} days</p>
			</div>
			<AreaChart
				{labels}
				series={[{ name: 'Stored', color: 'text-accent', values: storageSeries }]}
				format={(n) => bytes(n)}
				height={200}
			/>
		</Card>

		<Card>
			<h2 class="mb-4 text-sm font-medium">Jobs by state</h2>
			{#if jobs.length === 0}
				<p class="py-12 text-center text-sm text-muted">No jobs yet.</p>
			{:else}
				<div class="flex justify-center">
					<Donut segments={stateSegments} size={150}>
						{#snippet center()}
							<span class="mono text-2xl font-semibold">{jobs.length}</span>
							<span class="text-[10px] text-muted">total</span>
						{/snippet}
					</Donut>
				</div>
				<div class="mt-4 flex flex-col gap-2">
					{#each stateSegments as s (s.label)}
						<div class="flex items-center gap-2 text-xs">
							<span class={`h-2 w-2 rounded-full bg-current ${s.color}`}></span>
							<span class="text-muted">{s.label}</span>
							<span class="mono ml-auto font-medium">{s.value}</span>
						</div>
					{/each}
				</div>
			{/if}
		</Card>
	</div>

	<!-- Library breakdown -->
	<Card>
		<div class="mb-4 flex items-center justify-between">
			<h2 class="text-sm font-medium">Your library</h2>
			<span class="flex items-center gap-1.5 text-xs text-muted">
				<Icon icon={Timer} size={13} /> Avg processing
				<span class="mono font-semibold text-fg">{avgProcSecs === null ? '—' : fmtDuration(avgProcSecs)}</span>
			</span>
		</div>
		<div class="mb-4 flex items-baseline gap-2">
			<span class="mono text-3xl font-semibold">{assets.length}</span>
			<span class="text-sm text-muted">videos · {bytes(storageBytes)}</span>
		</div>
		<div class="grid gap-x-8 gap-y-2.5 sm:grid-cols-2">
			{#each assetStatus as s (s.label)}
				{@const frac = assets.length ? s.value / assets.length : 0}
				<div class="flex items-center gap-3">
					<span class="w-16 shrink-0 text-xs text-muted">{s.label}</span>
					<div class="h-2 flex-1 overflow-hidden rounded-full bg-surface-2">
						<div class={`h-full rounded-full bg-current ${s.color}`} style={`width:${frac * 100}%`}></div>
					</div>
					<span class="mono w-6 shrink-0 text-right text-xs">{s.value}</span>
				</div>
			{/each}
		</div>
	</Card>
</div>
