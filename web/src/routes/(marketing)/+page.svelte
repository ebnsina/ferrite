<script lang="ts">
	import { Icon } from '$lib/ui';
	import {
		Layers01Icon,
		DistributionIcon,
		ChipIcon,
		LiveStreaming01Icon,
		SecurityLockIcon,
		Image01Icon,
		CloudUploadIcon,
		QueueIcon,
		Rocket01Icon,
		ArrowRight01Icon
	} from '@hugeicons/core-free-icons';

	const features = [
		{
			icon: Layers01Icon,
			title: 'Adaptive HLS + DASH',
			body: 'One CMAF encode packages both HLS and DASH with shared fMP4 segments. Per-title renditions, ready for any player.'
		},
		{
			icon: DistributionIcon,
			title: 'Fair multi-tenant queue',
			body: "Round-robin scheduling with per-tenant in-flight caps. One tenant's 200 jobs never starve another's."
		},
		{
			icon: ChipIcon,
			title: 'CPU now, GPU-ready',
			body: 'Software encoding out of the box, with an NVENC path wired in for when you need hardware throughput.'
		},
		{
			icon: LiveStreaming01Icon,
			title: 'Live streaming',
			body: 'RTMP and SRT ingest, low-latency HTTP-FLV playback, and automatic archival of live streams to VOD.'
		},
		{
			icon: SecurityLockIcon,
			title: 'Signed playback',
			body: 'Private outputs served through a token-authorized proxy. Playlists are rewritten on the fly — no public buckets.'
		},
		{
			icon: Image01Icon,
			title: 'Thumbnails & sprites',
			body: 'Poster frames, sprite sheets, and WebVTT storyboards generated automatically for scrubbing previews.'
		}
	];

	const steps = [
		{
			icon: CloudUploadIcon,
			title: 'Upload',
			body: 'Push source video straight to S3-compatible storage with a presigned URL — no bytes through the API.'
		},
		{
			icon: QueueIcon,
			title: 'Transcode',
			body: 'Jobs enter the fair queue and fan out to workers. Track progress live over server-sent events.'
		},
		{
			icon: Rocket01Icon,
			title: 'Stream',
			body: 'Deliver adaptive HLS/DASH with signed URLs and rendition selection, on web, mobile, and TV.'
		}
	];
</script>

<svelte:head>
	<title>Ferrite — multi-tenant video transcoding</title>
	<meta
		name="description"
		content="Ferrite is a self-hosted, multi-tenant video transcoding platform. Adaptive HLS + DASH, a fair queue, live streaming, and signed playback."
	/>
</svelte:head>

<!-- Hero -->
<section class="relative overflow-hidden">
	<div
		class="pointer-events-none absolute inset-0 -z-10"
		style="background: radial-gradient(60% 50% at 50% 0%, var(--accent-soft) 0%, transparent 70%);"
	></div>
	<div class="mx-auto max-w-6xl px-6 py-24 text-center sm:py-32">
		<span
			class="mono inline-flex items-center gap-2 rounded-full border border-border bg-surface px-3 py-1 text-xs text-muted"
		>
			<span class="h-1.5 w-1.5 rounded-full bg-accent"></span> VOD &amp; live · self-hosted
		</span>
		<h1 class="mx-auto mt-6 max-w-3xl text-4xl font-semibold tracking-tight sm:text-6xl">
			Video transcoding that <span class="text-accent">scales with your tenants</span>
		</h1>
		<p class="mx-auto mt-6 max-w-2xl text-lg text-muted">
			Ferrite turns raw uploads into adaptive HLS and DASH — with a fair multi-tenant queue, live
			streaming, and signed playback. Self-hosted on your own S3 storage.
		</p>
		<div class="mt-10 flex flex-wrap items-center justify-center gap-3">
			<a
				href="/app"
				class="inline-flex items-center gap-2 rounded-lg bg-accent px-5 py-3 text-sm font-medium text-accent-fg transition-opacity hover:opacity-90"
			>
				Start transcoding <Icon icon={ArrowRight01Icon} size={16} />
			</a>
			<a
				href="/pricing"
				class="rounded-lg border border-border bg-surface px-5 py-3 text-sm font-medium transition-colors hover:bg-surface-2"
				>View pricing</a
			>
		</div>
		<p class="mono mt-6 text-xs text-muted">HLS · DASH · CMAF · AES-128 · NVENC-ready · SRT</p>
	</div>
</section>

<!-- Features -->
<section id="features" class="border-t border-border">
	<div class="mx-auto max-w-6xl px-6 py-20">
		<div class="mx-auto max-w-2xl text-center">
			<h2 class="text-3xl font-semibold tracking-tight">Everything the pipeline needs</h2>
			<p class="mt-3 text-muted">
				From ingest to adaptive delivery — built for scale and multi-tenancy from day one.
			</p>
		</div>
		<div class="mt-14 grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
			{#each features as f (f.title)}
				<div class="rounded-xl border border-border bg-surface p-6">
					<span
						class="mb-4 flex h-10 w-10 items-center justify-center rounded-lg bg-accent-soft text-accent"
					>
						<Icon icon={f.icon} size={20} />
					</span>
					<h3 class="text-base font-semibold">{f.title}</h3>
					<p class="mt-2 text-sm text-muted">{f.body}</p>
				</div>
			{/each}
		</div>
	</div>
</section>

<!-- How it works -->
<section id="how" class="border-t border-border bg-surface/40">
	<div class="mx-auto max-w-6xl px-6 py-20">
		<div class="mx-auto max-w-2xl text-center">
			<h2 class="text-3xl font-semibold tracking-tight">Three steps to adaptive video</h2>
			<p class="mt-3 text-muted">Upload a source file and Ferrite handles the rest.</p>
		</div>
		<div class="mt-14 grid gap-8 md:grid-cols-3">
			{#each steps as s, i (s.title)}
				<div class="relative">
					<div class="mb-4 flex items-center gap-3">
						<span
							class="flex h-10 w-10 items-center justify-center rounded-lg border border-border bg-surface text-accent"
						>
							<Icon icon={s.icon} size={20} />
						</span>
						<span class="mono text-sm text-muted">0{i + 1}</span>
					</div>
					<h3 class="text-lg font-semibold">{s.title}</h3>
					<p class="mt-2 text-sm text-muted">{s.body}</p>
				</div>
			{/each}
		</div>
	</div>
</section>

<!-- CTA -->
<section class="border-t border-border">
	<div class="mx-auto max-w-4xl px-6 py-20 text-center">
		<h2 class="text-3xl font-semibold tracking-tight">Spin up your workspace in minutes</h2>
		<p class="mx-auto mt-3 max-w-xl text-muted">
			Create an account, upload a video, and watch it become adaptive HLS and DASH.
		</p>
		<a
			href="/app"
			class="mt-8 inline-flex items-center gap-2 rounded-lg bg-accent px-5 py-3 text-sm font-medium text-accent-fg transition-opacity hover:opacity-90"
		>
			Get started free <Icon icon={ArrowRight01Icon} size={16} />
		</a>
	</div>
</section>
