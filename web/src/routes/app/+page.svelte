<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { Card, Button, StatusPill, ProgressBar, Icon } from '$lib/ui';
	import { listAssets, listJobs, getUsage } from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import { humanizeError } from '$lib/humanize';
	import type { Asset, Job, JobState, Usage } from '$lib/api/types';
	import { bytes, timeAgo, greeting, nameFromEmail, longDate } from '$lib/format';
	import { session } from '$lib/api/session.svelte';
	import { UploadSimple, FilmStrip, Queue, HardDrives, CheckCircle, ArrowRight, MagicWand, Scissors, MagnifyingGlass, Broadcast, Code, ClosedCaptioning, ShieldCheck, Lightning, CreditCard } from 'phosphor-svelte';

	const name = $derived(session.user?.name || nameFromEmail(session.user?.email));

	let assets = $state<Asset[]>([]);
	let jobs = $state<Job[]>([]);
	let usage = $state<Usage | null>(null);
	let error = $state<string | null>(null);
	let timer: ReturnType<typeof setInterval>;

	const ACTIVE: JobState[] = ['queued', 'probing', 'transcoding', 'packaging', 'uploading'];

	async function load() {
		try {
			[assets, jobs, usage] = await Promise.all([listAssets(), listJobs(), getUsage()]);
			error = null;
		} catch (e) {
			error = humanizeError(e instanceof ApiError ? e.message : null, 'We couldn’t load your dashboard.');
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
		completed.length + failed.length === 0 ? null : (completed.length / (completed.length + failed.length)) * 100
	);
	const storageBytes = $derived(usage?.storage_bytes ?? assets.reduce((s, a) => s + (a.bytes ?? 0), 0));

	const kpis = $derived([
		{ label: 'Videos', value: String(assets.length), icon: FilmStrip },
		{ label: 'Storage used', value: bytes(storageBytes), icon: HardDrives },
		{ label: 'Active jobs', value: String(inflight.length), icon: Queue },
		{ label: 'Success rate', value: successRate === null ? '—' : `${successRate.toFixed(0)}%`, icon: CheckCircle }
	]);

	const recent = $derived(jobs.slice(0, 6));

	const explore = [
		{ icon: MagicWand, title: 'AI shorts', href: '/app/assets' },
		{ icon: Scissors, title: 'Clip & trim', href: '/app/assets' },
		{ icon: ClosedCaptioning, title: 'Auto-captions', href: '/app/assets' },
		{ icon: MagnifyingGlass, title: 'Search in videos', href: '/app/search' },
		{ icon: Broadcast, title: 'Go live', href: '/app/live' },
		{ icon: Code, title: 'Embed & analytics', href: '/app/jobs' },
		{ icon: ShieldCheck, title: 'Content credentials', href: '/app/assets' },
		{ icon: Lightning, title: 'Watermark & DRM', href: '/app/assets' }
	];
</script>

<div class="mx-auto max-w-5xl">
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
		<a href="/app/assets"><Button><Icon icon={UploadSimple} size={16} /> Upload video</Button></a>
	</div>

	{#if error}
		<div class="mb-4 rounded-lg border border-danger/30 bg-danger/10 px-4 py-3 text-sm text-danger">{error}</div>
	{/if}

	<!-- KPIs -->
	<div class="mb-6 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
		{#each kpis as k (k.label)}
			<Card>
				<div class="flex items-center justify-between">
					<div>
						<p class="text-xs text-muted">{k.label}</p>
						<p class="mono mt-1 text-2xl font-semibold">{k.value}</p>
					</div>
					<span class="flex h-9 w-9 items-center justify-center rounded-lg bg-surface-2 text-muted">
						<Icon icon={k.icon} size={18} />
					</span>
				</div>
			</Card>
		{/each}
	</div>

	<!-- Recent jobs + active pipeline -->
	<div class="mb-6 grid gap-6 lg:grid-cols-3">
		<Card class="lg:col-span-2">
			<div class="mb-4 flex items-center justify-between">
				<h2 class="text-sm font-medium">Recent jobs</h2>
				<a href="/app/jobs" class="flex items-center gap-1 text-xs text-muted transition-colors hover:text-fg">
					View all <Icon icon={ArrowRight} size={13} />
				</a>
			</div>
			{#if recent.length === 0}
				<div class="flex flex-col items-center justify-center py-12 text-center">
					<span class="mb-3 text-muted"><Icon icon={Queue} size={30} /></span>
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
					<span class="mb-2 text-success"><Icon icon={CheckCircle} size={26} /></span>
					<p class="text-sm text-muted">You’re all caught up.</p>
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

	<!-- This month → nudge to Analytics -->
	{#if usage}
		{@const u = usage}
		<a href="/app/usage" class="mb-6 block">
			<Card class="transition-colors hover:border-accent/40 hover:bg-surface-2">
				<div class="flex flex-wrap items-center justify-between gap-4">
					<div class="flex items-center gap-8">
						<div>
							<p class="text-xs text-muted">This month</p>
							<p class="mono mt-0.5 text-xl font-semibold text-accent">${u.cost.total.toFixed(2)}</p>
						</div>
						<div>
							<p class="text-xs text-muted">Transcoded</p>
							<p class="mono mt-0.5 text-lg font-semibold">{u.minutes.toFixed(1)} min</p>
						</div>
						<div class="hidden sm:block">
							<p class="text-xs text-muted">Storage</p>
							<p class="mono mt-0.5 text-lg font-semibold">{bytes(u.storage_bytes)}</p>
						</div>
					</div>
					<span class="flex items-center gap-1.5 text-sm text-muted">
						<Icon icon={CreditCard} size={15} /> View usage
						<Icon icon={ArrowRight} size={14} />
					</span>
				</div>
			</Card>
		</a>
	{/if}

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
