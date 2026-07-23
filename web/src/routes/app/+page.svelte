<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { Card, Button, StatusPill, ProgressBar, Icon } from '$lib/ui';
	import { AreaChart, Donut, Gauge, Sparkline, Meter, bucketByDay, sumByDay, cumulative } from '$lib/charts';
	import { listAssets, listJobs, getUsage } from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import { humanizeError } from '$lib/humanize';
	import type { Asset, Job, JobState } from '$lib/api/types';
	import type { Usage } from '$lib/api/types';
	import { bytes, timeAgo, greeting, nameFromEmail, longDate } from '$lib/format';
	import { session } from '$lib/api/session.svelte';
	import {
		Upload01Icon,
		Film01Icon,
		PlayListIcon,
		HardDriveIcon,
		CheckmarkCircle02Icon,
		Timer01Icon,
		ArrowRight01Icon,
		AiVideoIcon,
		Scissor01Icon,
		Search01Icon,
		LiveStreaming01Icon,
		CodeIcon,
		SubtitleIcon,
		ShieldIcon,
		FlashIcon
	} from '@hugeicons/core-free-icons';

	const name = $derived(session.user?.name || nameFromEmail(session.user?.email));

	let assets = $state<Asset[]>([]);
	let jobs = $state<Job[]>([]);
	let usage = $state<Usage | null>(null);
	let error = $state<string | null>(null);
	let loaded = $state(false);
	let range = $state(14);
	let timer: ReturnType<typeof setInterval>;

	const ACTIVE: JobState[] = ['queued', 'probing', 'transcoding', 'packaging', 'uploading'];

	async function load() {
		try {
			[assets, jobs, usage] = await Promise.all([listAssets(), listJobs(), getUsage()]);
			error = null;
		} catch (e) {
			error = humanizeError(e instanceof ApiError ? e.message : null, 'We couldn’t load your dashboard.');
		} finally {
			loaded = true;
		}
	}

	onMount(() => {
		load();
		timer = setInterval(load, 5000);
	});
	onDestroy(() => clearInterval(timer));

	// --- Derived metrics (all from real data, no mocks) ----------------------
	const inflight = $derived(jobs.filter((j) => ACTIVE.includes(j.state)));
	const completed = $derived(jobs.filter((j) => j.state === 'completed'));
	const failed = $derived(jobs.filter((j) => j.state === 'failed'));
	const successRate = $derived(
		completed.length + failed.length === 0
			? 0
			: (completed.length / (completed.length + failed.length)) * 100
	);
	const storageBytes = $derived(usage?.storage_bytes ?? assets.reduce((s, a) => s + (a.bytes ?? 0), 0));

	// Average processing time for completed jobs.
	const avgProcSecs = $derived.by(() => {
		const done = completed.filter((j) => j.finished_at);
		if (!done.length) return null;
		const t = done.reduce(
			(s, j) => s + (new Date(j.finished_at!).getTime() - new Date(j.queued_at).getTime()),
			0
		);
		return t / done.length / 1000;
	});

	// Time series over the selected range.
	const labels = $derived(bucketByDay([], range).map((b) => b.label));
	const completedByDay = $derived(
		bucketByDay(completed.map((j) => j.finished_at ?? j.queued_at), range).map((b) => b.value)
	);
	const failedByDay = $derived(
		bucketByDay(failed.map((j) => j.finished_at ?? j.queued_at), range).map((b) => b.value)
	);
	const jobsByDay = $derived(bucketByDay(jobs.map((j) => j.queued_at), range).map((b) => b.value));
	const newAssetsByDay = $derived(
		bucketByDay(assets.map((a) => a.created_at), range).map((b) => b.value)
	);

	// Cumulative storage growth: assets before the window form the baseline.
	const storageSeries = $derived.by(() => {
		const start = new Date();
		start.setDate(start.getDate() - (range - 1));
		start.setHours(0, 0, 0, 0);
		const base = assets
			.filter((a) => new Date(a.created_at) < start)
			.reduce((s, a) => s + (a.bytes ?? 0), 0);
		const perDay = sumByDay(assets, (a) => a.created_at, (a) => a.bytes ?? 0, range).map((b) => b.value);
		return cumulative(perDay).map((v) => v + base);
	});

	// Job state distribution for the donut (grouped into three clear buckets).
	const stateSegments = $derived([
		{ label: 'Completed', value: completed.length, color: 'text-success' },
		{ label: 'In progress', value: inflight.length, color: 'text-accent' },
		{ label: 'Failed', value: failed.length, color: 'text-danger' }
	]);

	// Asset status distribution.
	const assetStatus = $derived([
		{ label: 'Ready', value: assets.filter((a) => a.status === 'ready').length, color: 'text-success' },
		{ label: 'Processing', value: assets.filter((a) => a.status === 'processing').length, color: 'text-accent' },
		{ label: 'Uploading', value: assets.filter((a) => a.status === 'uploading').length, color: 'text-warning' },
		{ label: 'Error', value: assets.filter((a) => a.status === 'error').length, color: 'text-danger' }
	]);

	// Cost composition (transcode vs storage).
	const costSegments = $derived(
		usage
			? [
					{ label: 'Transcode', value: usage.cost.transcode, color: 'text-accent' },
					{ label: 'Storage', value: usage.cost.storage, color: 'text-success' }
				]
			: []
	);

	function fmtDuration(secs: number): string {
		const s = Math.round(secs);
		if (s < 60) return `${s}s`;
		const m = Math.floor(s / 60);
		if (m < 60) return `${m}m ${s % 60}s`;
		return `${Math.floor(m / 60)}h ${m % 60}m`;
	}

	const kpis = $derived([
		{ label: 'Videos', value: String(assets.length), icon: Film01Icon, spark: newAssetsByDay, color: 'text-accent' },
		{ label: 'Storage used', value: bytes(storageBytes), icon: HardDriveIcon, spark: storageSeries, color: 'text-accent' },
		{ label: 'Jobs run', value: String(jobs.length), icon: PlayListIcon, spark: jobsByDay, color: 'text-accent' },
		{
			label: 'Success rate',
			value: completed.length + failed.length === 0 ? '—' : `${successRate.toFixed(0)}%`,
			icon: CheckmarkCircle02Icon,
			spark: completedByDay,
			color: 'text-success'
		}
	]);

	const recent = $derived(jobs.slice(0, 6));

	const explore = [
		{ icon: AiVideoIcon, title: 'AI shorts', href: '/app/assets' },
		{ icon: Scissor01Icon, title: 'Clip & trim', href: '/app/assets' },
		{ icon: SubtitleIcon, title: 'Auto-captions', href: '/app/assets' },
		{ icon: Search01Icon, title: 'Search in videos', href: '/app/search' },
		{ icon: LiveStreaming01Icon, title: 'Go live', href: '/app/live' },
		{ icon: CodeIcon, title: 'Embed & analytics', href: '/app/jobs' },
		{ icon: ShieldIcon, title: 'Content credentials', href: '/app/assets' },
		{ icon: FlashIcon, title: 'Watermark & DRM', href: '/app/assets' }
	];
</script>

<div class="mx-auto max-w-6xl">
	<!-- Header -->
	<div class="mb-8 flex flex-wrap items-end justify-between gap-4">
		<div>
			<p class="text-sm text-muted">{longDate()}</p>
			<h1 class="mt-1 text-2xl font-semibold tracking-tight">{greeting()}, {name}</h1>
			<p class="mt-1 text-sm text-muted">
				{#if inflight.length > 0}
					{inflight.length} video{inflight.length === 1 ? '' : 's'} processing right now.
				{:else if jobs.length > 0}
					All caught up — nothing in the queue.
				{:else}
					Upload your first video to get started.
				{/if}
			</p>
		</div>
		<a href="/app/assets"><Button><Icon icon={Upload01Icon} size={16} /> Upload video</Button></a>
	</div>

	{#if error}
		<div class="mb-4 rounded-lg border border-danger/30 bg-danger/10 px-4 py-3 text-sm text-danger">{error}</div>
	{/if}

	<!-- KPI row with sparklines -->
	<div class="mb-6 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
		{#each kpis as k (k.label)}
			<Card class="overflow-hidden">
				<div class="flex items-start justify-between">
					<div>
						<p class="text-xs text-muted">{k.label}</p>
						<p class="mono mt-1 text-2xl font-semibold">{k.value}</p>
					</div>
					<span class="text-muted"><Icon icon={k.icon} size={18} /></span>
				</div>
				<div class="-mx-5 -mb-5 mt-3">
					<Sparkline values={k.spark} color={k.color} height={44} />
				</div>
			</Card>
		{/each}
	</div>

	<!-- Processing activity + success gauge -->
	<div class="mb-6 grid gap-6 lg:grid-cols-3">
		<Card class="lg:col-span-2">
			<div class="mb-4 flex items-center justify-between">
				<div>
					<h2 class="text-sm font-medium">Processing activity</h2>
					<p class="text-xs text-muted">Completed vs failed per day</p>
				</div>
				<div class="flex items-center gap-1 rounded-lg border border-border bg-surface-2 p-0.5 text-xs">
					{#each [14, 30] as r (r)}
						<button
							onclick={() => (range = r)}
							class={`rounded-md px-2.5 py-1 transition-colors ${range === r ? 'bg-surface text-fg shadow-sm' : 'text-muted hover:text-fg'}`}
						>{r}d</button>
					{/each}
				</div>
			</div>
			<AreaChart
				labels={labels}
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
				<div>
					<p class="mono text-lg font-semibold text-success">{completed.length}</p>
					<p class="text-[10px] text-muted">Completed</p>
				</div>
				<div>
					<p class="mono text-lg font-semibold text-accent">{inflight.length}</p>
					<p class="text-[10px] text-muted">In progress</p>
				</div>
				<div>
					<p class="mono text-lg font-semibold text-danger">{failed.length}</p>
					<p class="text-[10px] text-muted">Failed</p>
				</div>
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
				labels={labels}
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

	<!-- Usage + library + pipeline -->
	<div class="mb-6 grid gap-6 lg:grid-cols-3">
		<!-- Usage this month -->
		<Card>
			<div class="mb-4 flex items-center justify-between">
				<h2 class="text-sm font-medium">This month</h2>
				<span class="text-[10px] text-muted">estimated</span>
			</div>
			{#if usage}
				{@const u = usage}
				<div class="mb-5 flex items-center gap-4">
					<Donut segments={costSegments} size={96} thickness={12}>
						{#snippet center()}
							<span class="mono text-base font-semibold text-accent">${u.cost.total.toFixed(0)}</span>
						{/snippet}
					</Donut>
					<div class="flex-1">
						<p class="text-xs text-muted">Estimated cost</p>
						<p class="mono text-2xl font-semibold text-accent">${u.cost.total.toFixed(2)}</p>
						<p class="mt-0.5 text-[10px] text-muted">mock billing</p>
					</div>
				</div>
				<div class="flex flex-col gap-3">
					<Meter label="Transcoded" value={`${u.minutes.toFixed(1)} min`} fraction={u.minutes / 2000} hint="of a 2,000-min reference" />
					<Meter label="Storage" value={bytes(u.storage_bytes)} fraction={u.storage_gb / 100} color="text-success" hint="of a 100 GB reference" />
				</div>
			{:else}
				<p class="py-8 text-center text-sm text-muted">Loading…</p>
			{/if}
		</Card>

		<!-- Library / asset status -->
		<Card>
			<h2 class="mb-4 text-sm font-medium">Your library</h2>
			<div class="mb-4 flex items-baseline gap-2">
				<span class="mono text-3xl font-semibold">{assets.length}</span>
				<span class="text-sm text-muted">videos · {bytes(storageBytes)}</span>
			</div>
			<div class="flex flex-col gap-2.5">
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
			<div class="mt-4 flex items-center justify-between border-t border-border pt-4">
				<span class="flex items-center gap-1.5 text-xs text-muted">
					<Icon icon={Timer01Icon} size={13} /> Avg processing
				</span>
				<span class="mono text-sm font-semibold">{avgProcSecs === null ? '—' : fmtDuration(avgProcSecs)}</span>
			</div>
		</Card>

		<!-- Active pipeline -->
		<Card>
			<div class="mb-4 flex items-center justify-between">
				<h2 class="text-sm font-medium">Active pipeline</h2>
				{#if inflight.length > 0}
					<span class="flex items-center gap-1.5 text-xs text-accent">
						<span class="relative flex h-2 w-2">
							<span class="absolute inline-flex h-full w-full animate-ping rounded-full bg-accent opacity-60"></span>
							<span class="relative inline-flex h-2 w-2 rounded-full bg-accent"></span>
						</span>
						live
					</span>
				{/if}
			</div>
			{#if inflight.length === 0}
				<div class="flex flex-col items-center justify-center py-10 text-center">
					<span class="mb-2 text-success"><Icon icon={CheckmarkCircle02Icon} size={26} /></span>
					<p class="text-sm text-muted">Nothing processing — you’re all caught up.</p>
				</div>
			{:else}
				<div class="flex flex-col gap-3">
					{#each inflight.slice(0, 5) as job (job.id)}
						<a href={`/app/jobs/${job.id}`} class="block">
							<div class="mb-1 flex items-center justify-between text-xs">
								<code class="mono truncate text-muted">{job.id.slice(0, 8)}</code>
								<span class="text-accent">{job.state}</span>
							</div>
							<ProgressBar value={job.progress} />
						</a>
					{/each}
				</div>
			{/if}
		</Card>
	</div>

	<!-- Recent jobs -->
	<Card class="mb-6">
		<div class="mb-4 flex items-center justify-between">
			<h2 class="text-sm font-medium">Recent jobs</h2>
			<a href="/app/jobs" class="flex items-center gap-1 text-xs text-muted transition-colors hover:text-fg">
				View all <Icon icon={ArrowRight01Icon} size={13} />
			</a>
		</div>
		{#if recent.length === 0}
			<div class="flex flex-col items-center justify-center py-10 text-center">
				<span class="mb-3 text-muted"><Icon icon={PlayListIcon} size={30} /></span>
				<p class="font-medium">No jobs yet</p>
				<p class="mt-1 text-sm text-muted">Upload a video to kick off your first transcode.</p>
			</div>
		{:else}
			<div class="flex flex-col divide-y divide-border">
				{#each recent as job (job.id)}
					<a href={`/app/jobs/${job.id}`} class="flex items-center gap-4 py-2.5 transition-colors hover:bg-surface-2">
						<code class="mono w-20 shrink-0 truncate text-xs text-muted">{job.id.slice(0, 8)}</code>
						<div class="min-w-0 flex-1">
							{#if ACTIVE.includes(job.state)}
								<ProgressBar value={job.progress} />
							{:else}
								<p class="text-xs text-muted">{timeAgo(job.finished_at ?? job.queued_at)}</p>
							{/if}
						</div>
						<StatusPill state={job.state} />
					</a>
				{/each}
			</div>
		{/if}
	</Card>

	<!-- Explore -->
	<div>
		<h2 class="mb-3 text-sm font-medium text-muted">Explore</h2>
		<div class="grid grid-cols-2 gap-3 sm:grid-cols-4">
			{#each explore as c (c.title)}
				<a
					href={c.href}
					class="group flex items-center gap-3 rounded-xl border border-border bg-surface p-4 transition-colors hover:border-accent/40 hover:bg-surface-2"
				>
					<span class="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-accent-soft text-accent">
						<Icon icon={c.icon} size={18} />
					</span>
					<span class="text-sm font-medium">{c.title}</span>
				</a>
			{/each}
		</div>
	</div>
</div>
