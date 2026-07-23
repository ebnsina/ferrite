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
		ArrowRight01Icon,
		ArrowDown01Icon,
		CheckmarkCircle02Icon
	} from '@hugeicons/core-free-icons';

	function scrollTo(id: string) {
		document.getElementById(id)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
	}

	const clients = ['NORTHWIND', 'LOOPTV', 'VAYU MEDIA', 'CANTOR', 'HELIX', 'ORBIT'];

	// Plain-language explainers for non-technical readers: what it is + why it matters.
	const benefits = [
		{
			icon: Layers01Icon,
			eyebrow: 'Smooth playback',
			title: 'Plays perfectly on any connection',
			plain:
				'Ferrite automatically makes several versions of your video at different qualities. The viewer’s player picks the best one for their internet speed, moment to moment.',
			benefit: 'No buffering, no spinning wheels — on a laptop, a phone, or a smart TV.'
		},
		{
			icon: DistributionIcon,
			eyebrow: 'Fair for everyone',
			title: 'Every customer gets their turn',
			plain:
				'When lots of videos are uploaded at once, Ferrite shares the processing evenly instead of working through one big pile first.',
			benefit: 'One customer uploading 200 videos never makes everyone else wait in line.'
		},
		{
			icon: LiveStreaming01Icon,
			eyebrow: 'Live, then on-demand',
			title: 'Go live and keep the replay',
			plain:
				'Broadcast a live stream through the same platform, and Ferrite quietly records it while it happens.',
			benefit: 'The moment your event ends, an on-demand version is ready to watch.'
		},
		{
			icon: SecurityLockIcon,
			eyebrow: 'Private by default',
			title: 'Your videos stay yours',
			plain:
				'Instead of public links, videos are served through signed links that expire — so they can’t be copied around or scraped.',
			benefit: 'Only the people you allow can press play.'
		}
	];

	const features = [
		{
			icon: Layers01Icon,
			title: 'Adaptive HLS + DASH',
			body: 'One CMAF encode packages both HLS and DASH with shared fMP4 segments. Per-title renditions, ready for any player.'
		},
		{
			icon: DistributionIcon,
			title: 'Fair, isolated queues',
			body: "Round-robin scheduling with per-customer caps — one big workload can't starve everyone else's."
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

	const stats = [
		{ value: '12M+', label: 'minutes transcoded' },
		{ value: '40+', label: 'renditions / second' },
		{ value: '99.95%', label: 'delivery uptime' },
		{ value: '<5 min', label: 'upload to stream' }
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

	const testimonials = [
		{
			quote:
				'Ferrite cut our transcode pipeline from a weekend project to an afternoon. It just does the right thing.',
			name: 'Maya Chen',
			role: 'Head of Video, LoopTV'
		},
		{
			quote:
				"The fair queue is the killer feature. Our biggest customer can't drown out everyone else anymore.",
			name: 'Diego Alvarez',
			role: 'Platform Lead, Northwind'
		},
		{
			quote:
				'We replaced three services with one. HLS, DASH, and thumbnails out of a single upload.',
			name: 'Priya Nair',
			role: 'CTO, Vayu Media'
		}
	];

	const tiers = [
		{
			name: 'Hobby',
			price: '$0',
			cadence: '/mo',
			blurb: 'For side projects and evaluation.',
			cta: 'Start free',
			highlight: false,
			features: [
				'1 workspace',
				'CPU transcoding',
				'HLS + DASH output',
				'5 GB storage',
				'Community support'
			]
		},
		{
			name: 'Pro',
			price: '$49',
			cadence: '/mo',
			blurb: 'For teams shipping video to production.',
			cta: 'Get started',
			highlight: true,
			features: [
				'Unlimited team members',
				'Priority fair-queue lane',
				'Live streaming (RTMP + SRT)',
				'500 GB storage',
				'Signed playback URLs',
				'Email support'
			]
		},
		{
			name: 'Scale',
			price: 'Custom',
			cadence: '',
			blurb: 'For high-volume video platforms.',
			cta: 'Contact us',
			highlight: false,
			features: [
				'GPU (NVENC) encoding',
				'Dedicated workers',
				'Custom storage & regions',
				'DRM & advanced security',
				'SLA & priority support'
			]
		}
	];

	const faqs = [
		{
			q: 'Can I run Ferrite on my own infrastructure?',
			a: 'Yes. Ferrite is self-hosted and stores everything in your own S3-compatible bucket — MinIO, AWS S3, or any compatible provider.'
		},
		{
			q: 'Which output formats are supported?',
			a: 'Adaptive HLS and DASH from a single CMAF encode, plus AES-128 encrypted HLS, poster frames, sprite sheets, and WebVTT storyboards.'
		},
		{
			q: 'Do I need a GPU?',
			a: 'No. CPU encoding works out of the box. When you need more throughput, the NVENC hardware path is already wired in.'
		},
		{
			q: 'Does it handle live streaming?',
			a: 'Yes — RTMP and SRT ingest with low-latency playback, and live streams are automatically archived to VOD.'
		}
	];
</script>

<svelte:head>
	<title>Ferrite — adaptive video transcoding at scale</title>
	<meta
		name="description"
		content="Ferrite turns raw uploads into adaptive HLS and DASH — with live streaming, a fair queue, and signed playback. Self-hosted on your own S3 storage."
	/>
</svelte:head>

<!-- Hero -->
<section class="relative overflow-hidden">
	<div
		class="pointer-events-none absolute inset-0 -z-10"
		style="background: radial-gradient(70% 55% at 50% -5%, var(--accent-soft) 0%, transparent 70%);"
	></div>
	<div class="mx-auto max-w-6xl px-6 pt-40 pb-24 text-center sm:pt-48 sm:pb-32">
		<h1 class="mx-auto max-w-3xl text-4xl font-semibold tracking-tight sm:text-6xl">
			Encode once. <span class="text-accent">Stream everywhere.</span>
		</h1>
		<p class="mx-auto mt-6 max-w-2xl text-lg text-muted">
			Ferrite turns raw uploads into adaptive HLS and DASH in minutes — with live streaming, signed
			playback, and a queue that keeps every workload moving.
		</p>
		<div class="mt-10 flex flex-wrap items-center justify-center gap-3">
			<a
				href="/app"
				class="inline-flex items-center gap-2 rounded-lg bg-accent px-5 py-3 text-sm font-medium text-accent-fg transition-opacity hover:opacity-90"
			>
				Start transcoding <Icon icon={ArrowRight01Icon} size={16} />
			</a>
			<button
				onclick={() => scrollTo('benefits')}
				class="rounded-lg border border-border bg-surface px-5 py-3 text-sm font-medium transition-colors hover:bg-surface-2"
				>See how it helps</button
			>
		</div>
	</div>
</section>

<!-- Client logo cloud -->
<section class="border-t border-border">
	<div class="mx-auto max-w-6xl px-6 py-12">
		<p class="text-center text-xs font-medium tracking-wide text-muted uppercase">
			Powering video for teams everywhere
		</p>
		<div class="mt-6 flex flex-wrap items-center justify-center gap-x-10 gap-y-4">
			{#each clients as c (c)}
				<span class="text-sm font-semibold tracking-widest text-muted/70">{c}</span>
			{/each}
		</div>
	</div>
</section>

<!-- Why Ferrite — plain-language benefits for non-technical readers -->
<section id="benefits" class="border-t border-border">
	<div class="mx-auto max-w-6xl px-6 py-20">
		<div class="mx-auto max-w-2xl text-center">
			<h2 class="text-3xl font-semibold tracking-tight">What Ferrite does, in plain English</h2>
			<p class="mt-3 text-muted">
				No jargon. Here's what each part means for you and your viewers.
			</p>
		</div>
		<div class="mt-16 flex flex-col gap-16">
			{#each benefits as b, i (b.title)}
				<div class="grid items-center gap-8 md:grid-cols-2">
					<div class={i % 2 === 1 ? 'md:order-2' : ''}>
						<span class="text-xs font-semibold tracking-wide text-accent uppercase">{b.eyebrow}</span
						>
						<h3 class="mt-2 text-2xl font-semibold tracking-tight">{b.title}</h3>
						<p class="mt-3 text-muted">{b.plain}</p>
						<p class="mt-4 flex items-start gap-2 text-sm font-medium">
							<span class="mt-0.5 text-accent"><Icon icon={CheckmarkCircle02Icon} size={16} /></span>
							{b.benefit}
						</p>
					</div>
					<div class={i % 2 === 1 ? 'md:order-1' : ''}>
						<div
							class="flex aspect-[4/3] items-center justify-center rounded-2xl border border-border bg-surface"
							style="background-image: radial-gradient(60% 60% at 50% 40%, var(--accent-soft) 0%, transparent 70%);"
						>
							<span class="text-accent"><Icon icon={b.icon} size={72} /></span>
						</div>
					</div>
				</div>
			{/each}
		</div>
	</div>
</section>

<!-- Features -->
<section id="features" class="border-t border-border bg-surface/40">
	<div class="mx-auto max-w-6xl px-6 py-20">
		<div class="mx-auto max-w-2xl text-center">
			<h2 class="text-3xl font-semibold tracking-tight">Everything the pipeline needs</h2>
			<p class="mt-3 text-muted">
				From ingest to adaptive delivery — built for scale and isolation from day one.
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

<!-- Stats band -->
<section class="border-t border-border">
	<div class="mx-auto grid max-w-6xl grid-cols-2 gap-8 px-6 py-16 lg:grid-cols-4">
		{#each stats as s (s.label)}
			<div class="text-center">
				<p class="text-4xl font-semibold tracking-tight text-accent">{s.value}</p>
				<p class="mt-2 text-sm text-muted">{s.label}</p>
			</div>
		{/each}
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
				<div>
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

<!-- Testimonials -->
<section class="border-t border-border">
	<div class="mx-auto max-w-6xl px-6 py-20">
		<div class="mx-auto max-w-2xl text-center">
			<h2 class="text-3xl font-semibold tracking-tight">Teams ship faster on Ferrite</h2>
			<p class="mt-3 text-muted">What engineering and video teams say after switching.</p>
		</div>
		<div class="mt-14 grid gap-6 md:grid-cols-3">
			{#each testimonials as t (t.name)}
				<figure class="flex flex-col rounded-xl border border-border bg-surface p-6">
					<blockquote class="flex-1 text-sm leading-relaxed">"{t.quote}"</blockquote>
					<figcaption class="mt-5 flex items-center gap-3">
						<span
							class="flex h-9 w-9 items-center justify-center rounded-full bg-accent-soft text-sm font-semibold text-accent"
							>{t.name.charAt(0)}</span
						>
						<span>
							<span class="block text-sm font-medium">{t.name}</span>
							<span class="block text-xs text-muted">{t.role}</span>
						</span>
					</figcaption>
				</figure>
			{/each}
		</div>
	</div>
</section>

<!-- Pricing -->
<section id="pricing" class="border-t border-border bg-surface/40">
	<div class="mx-auto max-w-6xl px-6 py-20">
		<div class="mx-auto max-w-2xl text-center">
			<h2 class="text-3xl font-semibold tracking-tight">Simple, scalable pricing</h2>
			<p class="mt-3 text-muted">
				Start free, upgrade when you grow. Billing is mocked in this demo environment.
			</p>
		</div>
		<div class="mt-14 grid gap-6 lg:grid-cols-3">
			{#each tiers as t (t.name)}
				<div
					class={`flex flex-col rounded-xl border p-6 ${
						t.highlight ? 'border-accent bg-surface ring-1 ring-accent' : 'border-border bg-surface'
					}`}
				>
					<div class="flex items-center justify-between">
						<h3 class="text-lg font-semibold">{t.name}</h3>
						{#if t.highlight}
							<span class="mono rounded-full bg-accent-soft px-2 py-0.5 text-xs text-accent"
								>Popular</span
							>
						{/if}
					</div>
					<p class="mt-1 text-sm text-muted">{t.blurb}</p>
					<div class="mt-5 flex items-end gap-1">
						<span class="text-4xl font-semibold tracking-tight">{t.price}</span>
						{#if t.cadence}<span class="mb-1 text-sm text-muted">{t.cadence}</span>{/if}
					</div>
					<a
						href="/app"
						class={`mt-6 inline-flex items-center justify-center gap-2 rounded-lg px-4 py-2.5 text-sm font-medium transition-colors ${
							t.highlight
								? 'bg-accent text-accent-fg hover:opacity-90'
								: 'border border-border hover:bg-surface-2'
						}`}
					>
						{t.cta} <Icon icon={ArrowRight01Icon} size={15} />
					</a>
					<ul class="mt-6 flex flex-col gap-3 border-t border-border pt-6">
						{#each t.features as f (f)}
							<li class="flex items-start gap-2.5 text-sm">
								<span class="mt-0.5 text-accent"
									><Icon icon={CheckmarkCircle02Icon} size={16} /></span
								>
								<span>{f}</span>
							</li>
						{/each}
					</ul>
				</div>
			{/each}
		</div>
	</div>
</section>

<!-- FAQ -->
<section class="border-t border-border">
	<div class="mx-auto max-w-3xl px-6 py-20">
		<h2 class="text-center text-3xl font-semibold tracking-tight">Frequently asked</h2>
		<div class="mt-10 divide-y divide-border">
			{#each faqs as f (f.q)}
				<details class="group py-4">
					<summary
						class="flex cursor-pointer list-none items-center justify-between text-base font-medium"
					>
						{f.q}
						<span class="text-muted transition-transform group-open:rotate-180">
							<Icon icon={ArrowDown01Icon} size={18} />
						</span>
					</summary>
					<p class="mt-3 text-sm text-muted">{f.a}</p>
				</details>
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
