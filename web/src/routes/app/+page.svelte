<script lang="ts">
	import { onMount } from 'svelte';
	import { Card, Button, StatusPill, ProgressBar, Icon } from '$lib/ui';
	import { listAssets, listJobs, getUsage } from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import type { Asset, Job, JobState, Usage } from '$lib/api/types';
	import { bytes, timeAgo, greeting, nameFromEmail, longDate } from '$lib/format';
	import { session } from '$lib/api/session.svelte';
	import {
		Upload01Icon,
		Film01Icon,
		PlayListIcon,
		HardDriveIcon,
		AiVideoIcon,
		Scissor01Icon,
		SubtitleIcon,
		SecurityLockIcon,
		LiveStreaming01Icon,
		CodeIcon,
		Search01Icon,
		ShieldIcon,
		ArrowRight01Icon
	} from '@hugeicons/core-free-icons';

	const name = $derived(session.user?.name || nameFromEmail(session.user?.email));

	// Feature entry points — mirror what the landing advertises so it's all
	// discoverable from the dashboard.
	const capabilities = [
		{
			icon: Upload01Icon,
			title: 'Upload & transcode',
			body: 'Adaptive HLS + DASH, MP4/audio downloads, captions & watermark options.',
			href: '/app/assets'
		},
		{
			icon: AiVideoIcon,
			title: 'AI vertical shorts',
			body: 'Turn a long video into 9:16 shorts with auto-picked highlights + captions.',
			href: '/app/assets'
		},
		{
			icon: Scissor01Icon,
			title: 'Clip & trim',
			body: 'Cut any section of a source into a new asset in seconds.',
			href: '/app/assets'
		},
		{
			icon: SubtitleIcon,
			title: 'Auto-captions',
			body: 'Transcribe speech to a WebVTT track — local or provider-agnostic.',
			href: '/app/assets'
		},
		{
			icon: Search01Icon,
			title: 'Search inside videos',
			body: 'Search the spoken words across your library and jump to the moment.',
			href: '/app/search'
		},
		{
			icon: ShieldIcon,
			title: 'Content credentials',
			body: 'Signed, tamper-evident provenance + edit lineage on produced assets.',
			href: '/app/assets'
		},
		{
			icon: LiveStreaming01Icon,
			title: 'Live & simulcast',
			body: 'Go live, restream to YouTube/Twitch, and clip moments in real time.',
			href: '/app/live'
		},
		{
			icon: CodeIcon,
			title: 'Embed & analytics',
			body: 'Grab an embeddable player and track views, watch-time & completion.',
			href: '/app/jobs'
		}
	];

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
			<h1 class="mt-1 text-2xl font-semibold tracking-tight">{greeting()}, {name}</h1>
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

	<!-- Capabilities — discoverable entry points for everything Ferrite does. -->
	<div class="mb-8">
		<h2 class="mb-3 text-sm font-medium text-muted">What you can do</h2>
		<div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
			{#each capabilities as c (c.title)}
				<a
					href={c.href}
					class="group rounded-xl border border-border bg-surface p-5 transition-colors hover:border-accent/40 hover:bg-surface-2"
				>
					<div class="flex items-start justify-between">
						<span
							class="flex h-10 w-10 items-center justify-center rounded-lg bg-accent-soft text-accent"
						>
							<Icon icon={c.icon} size={20} />
						</span>
						<span class="text-muted transition-transform group-hover:translate-x-0.5">
							<Icon icon={ArrowRight01Icon} size={16} />
						</span>
					</div>
					<h3 class="mt-4 text-sm font-semibold">{c.title}</h3>
					<p class="mt-1 text-xs text-muted">{c.body}</p>
				</a>
			{/each}
		</div>
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
