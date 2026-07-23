<script lang="ts">
	import { onMount } from 'svelte';
	import { Card, Button, StatusPill, ProgressBar, Icon } from '$lib/ui';
	import { listAssets, listJobs, getUsage } from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import type { Asset, Job, JobState, Usage } from '$lib/api/types';
	import { bytes, timeAgo, greeting, nameFromEmail, longDate } from '$lib/format';
	import { session } from '$lib/api/session.svelte';
	import { Upload01Icon, Film01Icon, PlayListIcon, HardDriveIcon } from '@hugeicons/core-free-icons';

	const name = $derived(nameFromEmail(session.user?.email));

	let assets = $state<Asset[]>([]);
	let jobs = $state<Job[]>([]);
	let usage = $state<Usage | null>(null);
	let error = $state<string | null>(null);

	const ACTIVE: JobState[] = ['queued', 'probing', 'transcoding', 'packaging', 'uploading'];
	const activeCount = $derived(jobs.filter((j) => ACTIVE.includes(j.state)).length);
	const totalBytes = $derived(assets.reduce((s, a) => s + (a.bytes ?? 0), 0));
	const recent = $derived(jobs.slice(0, 5));

	onMount(async () => {
		try {
			[assets, jobs, usage] = await Promise.all([listAssets(), listJobs(), getUsage()]);
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Failed to load dashboard.';
		}
	});

	const stats = $derived([
		{ label: 'Assets', value: String(assets.length), icon: Film01Icon },
		{ label: 'Active jobs', value: String(activeCount), icon: PlayListIcon },
		{ label: 'Storage', value: bytes(totalBytes), icon: HardDriveIcon }
	]);
</script>

<div class="mx-auto max-w-5xl">
	<div class="mb-8 flex flex-wrap items-end justify-between gap-4">
		<div>
			<p class="text-sm text-muted">{longDate()}</p>
			<h1 class="mt-1 text-2xl font-semibold tracking-tight">
				{greeting()}, {name} <span class="text-accent">👋</span>
			</h1>
			<p class="mt-1 text-sm text-muted">
				{#if activeCount > 0}
					{activeCount} job{activeCount === 1 ? '' : 's'} transcoding right now.
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

	<div class="mb-8 grid gap-4 sm:grid-cols-3">
		{#each stats as stat (stat.label)}
			<Card>
				<div class="flex items-center justify-between">
					<div>
						<p class="text-sm text-muted">{stat.label}</p>
						<p class="mono mt-1 text-2xl font-semibold">{stat.value}</p>
					</div>
					<span class="text-muted"><Icon icon={stat.icon} size={22} /></span>
				</div>
			</Card>
		{/each}
	</div>

	{#if usage}
		<Card class="mb-6">
			<div class="flex items-center justify-between">
				<h2 class="text-sm font-medium text-muted">This month</h2>
				<span class="text-xs text-muted">estimated · mock billing</span>
			</div>
			<div class="mt-4 flex flex-wrap items-end justify-between gap-4">
				<div class="flex gap-8">
					<div>
						<p class="text-xs text-muted">Transcoded</p>
						<p class="mono mt-1 text-lg font-semibold">{usage.minutes.toFixed(1)} min</p>
					</div>
					<div>
						<p class="text-xs text-muted">Storage</p>
						<p class="mono mt-1 text-lg font-semibold">{bytes(usage.storage_bytes)}</p>
					</div>
				</div>
				<div class="text-right">
					<p class="text-xs text-muted">Estimated cost</p>
					<p class="mono mt-1 text-2xl font-semibold text-accent">
						${usage.cost.total.toFixed(2)}
					</p>
				</div>
			</div>
		</Card>
	{/if}

	<Card>
		<h2 class="mb-4 text-sm font-medium text-muted">Recent jobs</h2>
		{#if recent.length === 0}
			<div class="flex flex-col items-center justify-center py-12 text-center">
				<span class="mb-3 text-muted"><Icon icon={PlayListIcon} size={32} /></span>
				<p class="font-medium">No jobs yet</p>
				<p class="mt-1 text-sm text-muted">Upload a video to kick off your first transcode.</p>
			</div>
		{:else}
			<div class="flex flex-col divide-y divide-border">
				{#each recent as job (job.id)}
					<div class="flex items-center gap-4 py-3">
						<code class="mono w-20 shrink-0 truncate text-xs text-muted">{job.id.slice(0, 8)}</code>
						<div class="min-w-0 flex-1">
							{#if ACTIVE.includes(job.state)}
								<ProgressBar value={job.progress} />
							{:else}
								<p class="text-xs text-muted">{timeAgo(job.queued_at)}</p>
							{/if}
						</div>
						<StatusPill state={job.state} />
					</div>
				{/each}
			</div>
		{/if}
	</Card>
</div>
