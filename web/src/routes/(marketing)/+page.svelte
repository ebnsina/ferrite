<script lang="ts">
	import { Icon } from '$lib/ui';
	import { reveal } from '$lib/actions/reveal';
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
		CheckmarkCircle02Icon,
		AiVideoIcon,
		SubtitleIcon,
		Scissor01Icon,
		CodeIcon,
		Share08Icon,
		ServerStack01Icon,
		DatabaseIcon,
		Coins01Icon,
		PackageIcon,
		CpuIcon,
		Cancel01Icon,
		Search01Icon,
		ShieldIcon
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
			benefit: 'One customer uploading 10,000 videos never makes everyone else wait in line.'
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
			body: 'One CMAF encode packages both HLS and DASH with shared fMP4 segments — ready for any player.'
		},
		{
			icon: AiVideoIcon,
			title: 'AI vertical shorts',
			body: 'Auto-find highlights from the transcript, reframe to 9:16, and burn in captions. Provider-agnostic — cloud or fully local.'
		},
		{
			icon: SubtitleIcon,
			title: 'Auto-captions',
			body: 'Transcribe speech to WebVTT with whisper.cpp locally or any OpenAI-compatible endpoint. Nothing leaves your box unless you want it to.'
		},
		{
			icon: ChipIcon,
			title: 'Per-title encoding',
			body: 'Content-aware bitrate ladders tailored to each source — cut egress on simple videos without touching quality on complex ones.'
		},
		{
			icon: Scissor01Icon,
			title: 'Clip, trim & thumbnails',
			body: 'Cut clips into new assets, extract a frame at any timestamp, and hover-to-play animated previews — all on demand.'
		},
		{
			icon: SecurityLockIcon,
			title: 'Watermark & signed playback',
			body: 'Burn your logo onto the stream + MP4, and serve private outputs through expiring signed tokens — no public buckets.'
		},
		{
			icon: LiveStreaming01Icon,
			title: 'Live + simulcast',
			body: 'RTMP/SRT ingest, low-latency playback, auto-archival to VOD, instant live clipping, and restream to YouTube/Twitch at once.'
		},
		{
			icon: CodeIcon,
			title: 'Embeddable player + analytics',
			body: 'A branded, signed <iframe> player with rendition selection, plus views, watch-time, and completion analytics.'
		},
		{
			icon: DistributionIcon,
			title: 'Fair multi-tenant queue',
			body: "Round-robin scheduling with per-customer caps — one tenant's 10,000 jobs can't starve everyone else's."
		}
	];

	// Ferrite's honest differentiators vs hosted video SaaS.
	const reasons = [
		{
			icon: ServerStack01Icon,
			title: 'Self-hosted, your rules',
			body: 'Run it on your own servers — cloud, on-prem, or fully air-gapped. No third party sits between you and your video.'
		},
		{
			icon: DatabaseIcon,
			title: 'Your storage, your data',
			body: 'Everything lives in your own S3-compatible bucket. Your originals and outputs never become someone else’s asset.'
		},
		{
			icon: Coins01Icon,
			title: 'No per-minute lock-in',
			body: 'Pay for your own infrastructure, not per delivered minute. Costs stay predictable as you scale.'
		},
		{
			icon: CpuIcon,
			title: 'Provider-agnostic AI',
			body: 'Captions and AI shorts run against local models or any OpenAI-compatible API — swap providers with one env var.'
		},
		{
			icon: PackageIcon,
			title: 'All-in-one pipeline',
			body: 'VOD, live, clipping, captions, AI shorts, embeds, and analytics in one system — no bolt-on services to wire together.'
		},
		{
			icon: SecurityLockIcon,
			title: 'Private by design',
			body: 'Signed playback, per-tenant isolation, and AES-128 encryption built in. Your content stays yours.'
		}
	];

	// Differentiators the hosted platforms don't offer.
	const differentiators = [
		{
			icon: Search01Icon,
			title: 'Search inside your videos',
			body: 'Search the spoken words across your entire library and jump straight to the exact moment.'
		},
		{
			icon: ShieldIcon,
			title: 'Content credentials',
			body: 'Ed25519-signed, tamper-evident provenance with full edit lineage on every produced asset.'
		},
		{
			icon: SubtitleIcon,
			title: 'Multi-language captions',
			body: 'Translate transcripts into any language, delivered as selectable caption tracks.'
		},
		{
			icon: SecurityLockIcon,
			title: 'On-ingest moderation',
			body: 'Automatically classify spoken content for policy safety as videos come in — locally.'
		},
		{
			icon: CodeIcon,
			title: 'Interactive transcript',
			body: 'A clickable transcript synced to playback — click a line to seek, share any moment.'
		},
		{
			icon: CpuIcon,
			title: 'Provider-agnostic AI',
			body: 'Every AI feature runs against local models or your own key — swap providers with one env var.'
		}
	];

	// Comparison — model-level differences, framed honestly against hosted platforms.
	const compareCols = ['Ferrite', 'Mux', 'api.video', 'Cloudflare Stream'];
	const compareRows = [
		{ label: 'Self-hosted / on-prem', cells: [true, false, false, false] },
		{ label: 'Store in your own S3 bucket', cells: [true, false, false, false] },
		{ label: 'No per-minute delivery fees', cells: [true, false, false, false] },
		{ label: 'In-video search', cells: [true, false, false, false] },
		{ label: 'Content provenance / credentials', cells: [true, false, false, false] },
		{ label: 'Provider-agnostic / local AI', cells: [true, false, false, false] },
		{ label: 'Runs fully offline / air-gapped', cells: [true, false, false, false] },
		{ label: 'Adaptive HLS + DASH', cells: [true, true, true, true] },
		{ label: 'Live streaming', cells: [true, true, true, true] },
		{ label: 'Embeddable player + analytics', cells: [true, true, true, true] }
	];

	// Friendly, grouped explanations of the jargon — what it means for you, not a spec.
	const glossary = [
		{ term: 'HLS + DASH', desc: 'The two ways video streams to phones, TVs, and browsers. We make both from one upload.' },
		{ term: 'ABR', desc: 'The picture quality adjusts itself to each viewer’s connection — no spinning wheel.' },
		{ term: 'CMAF', desc: 'Lets us build HLS and DASH from a single file, so you encode once instead of twice.' },
		{ term: 'RTMP + SRT', desc: 'The two common ways to send a live stream in. SRT keeps going even on shaky WiFi.' },
		{ term: 'AES-128', desc: 'Locks each piece of the video so only the viewers you approve can press play.' },
		{ term: 'NVENC', desc: 'Encodes on the graphics card — the same job, done much faster when you need speed.' }
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
			name: 'Free',
			price: '$0',
			cadence: '',
			blurb: 'Self-host it on your own infrastructure, forever.',
			cta: 'Get the code',
			highlight: false,
			caps: null,
			capsNote: 'Your infrastructure — your limits.',
			features: [
				'Full source-available core',
				'Your own S3 storage',
				'All core features',
				'Community support'
			]
		},
		{
			name: 'Starter',
			price: '$29',
			cadence: '/mo',
			blurb: 'Managed cloud for side projects & small apps.',
			cta: 'Start free trial',
			highlight: false,
			caps: { storage: '50 GB', minutes: '500 min', delivery: '100 GB' },
			capsNote: '',
			features: ['Adaptive HLS + DASH', 'Clips & thumbnails', 'Signed playback', 'Email support']
		},
		{
			name: 'Pro',
			price: '$99',
			cadence: '/mo',
			blurb: 'For teams shipping video to production.',
			cta: 'Start free trial',
			highlight: true,
			caps: { storage: '500 GB', minutes: '3,000 min', delivery: '1 TB' },
			capsNote: '',
			features: [
				'Everything in Starter',
				'AI shorts & captions',
				'In-video search',
				'Live + simulcast',
				'Analytics & embeds'
			]
		},
		{
			name: 'Enterprise',
			price: 'Custom',
			cadence: '',
			blurb: 'Self-hosted or dedicated, with the controls you need.',
			cta: 'Contact us',
			highlight: false,
			caps: null,
			capsNote: 'Custom limits, regions & SLA.',
			features: [
				'SSO / SAML & RBAC',
				'DRM & audit logs',
				'Content provenance at scale',
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
			Encode once.<br /><span class="text-accent">Stream everywhere.</span>
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
	<div class="mx-auto max-w-6xl px-6 py-20" use:reveal>
		<div class="mx-auto max-w-2xl text-center">
			<h2 class="text-3xl font-semibold tracking-tight">The hard parts of video, handled</h2>
			<p class="mt-3 text-muted">
				Encoding, delivery, and playback that just work — so you can focus on your content.
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

		<!-- The jargon, in human terms — cards, not a spec sheet. -->
		<div class="mt-16">
			<p class="text-center text-muted">
				You'll see a few acronyms around. Here's all you need to know:
			</p>
			<div class="mt-8 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
				{#each glossary as g (g.term)}
					<div class="rounded-xl border border-border bg-surface p-5">
						<span class="mono text-sm font-semibold text-accent">{g.term}</span>
						<p class="mt-2 text-sm text-muted">{g.desc}</p>
					</div>
				{/each}
			</div>
		</div>
	</div>
</section>

<!-- Features -->
<section id="features" class="border-t border-border bg-surface/40">
	<div class="mx-auto max-w-6xl px-6 py-20" use:reveal>
		<div class="mx-auto max-w-2xl text-center">
			<h2 class="text-3xl font-semibold tracking-tight">One platform, the whole pipeline</h2>
			<p class="mt-3 text-muted">
				VOD, live, AI, and delivery — everything you'd otherwise stitch together from five services.
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

<!-- Why Ferrite -->
<section id="why" class="border-t border-border">
	<div class="mx-auto max-w-6xl px-6 py-20" use:reveal>
		<div class="mx-auto max-w-2xl text-center">
			<h2 class="text-3xl font-semibold tracking-tight">Why teams choose Ferrite</h2>
			<p class="mt-3 text-muted">
				Same modern feature set as the hosted platforms — without handing over your video or your bill.
			</p>
		</div>
		<div class="mt-14 grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
			{#each reasons as r (r.title)}
				<div class="rounded-xl border border-border bg-surface p-6">
					<span
						class="mb-4 flex h-10 w-10 items-center justify-center rounded-lg bg-accent-soft text-accent"
					>
						<Icon icon={r.icon} size={20} />
					</span>
					<h3 class="text-base font-semibold">{r.title}</h3>
					<p class="mt-2 text-sm text-muted">{r.body}</p>
				</div>
			{/each}
		</div>
	</div>
</section>

<!-- Differentiators -->
<section class="border-t border-border">
	<div class="mx-auto max-w-6xl px-6 py-20" use:reveal>
		<div class="mx-auto max-w-2xl text-center">
			<span class="text-xs font-semibold tracking-wide text-accent uppercase">Only on Ferrite</span>
			<h2 class="mt-2 text-3xl font-semibold tracking-tight">Features no one else ships</h2>
			<p class="mt-3 text-muted">
				The parts the hosted platforms don't have — searchable, verifiable, and private by design.
			</p>
		</div>
		<div class="mt-14 grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
			{#each differentiators as d (d.title)}
				<div class="rounded-xl border border-accent/20 bg-accent-soft/30 p-6">
					<span
						class="mb-4 flex h-10 w-10 items-center justify-center rounded-lg bg-accent text-accent-fg"
					>
						<Icon icon={d.icon} size={20} />
					</span>
					<h3 class="text-base font-semibold">{d.title}</h3>
					<p class="mt-2 text-sm text-muted">{d.body}</p>
				</div>
			{/each}
		</div>
	</div>
</section>

<!-- Comparison -->
<section class="border-t border-border bg-surface/40">
	<div class="mx-auto max-w-4xl px-6 py-20" use:reveal>
		<div class="mx-auto max-w-2xl text-center">
			<h2 class="text-3xl font-semibold tracking-tight">Ferrite vs hosted platforms</h2>
			<p class="mt-3 text-muted">
				Where a self-hosted pipeline changes the equation. Feature parity where it counts.
			</p>
		</div>
		<div class="mt-12 overflow-x-auto">
			<table class="w-full min-w-[560px] border-collapse text-sm">
				<thead>
					<tr class="border-b border-border">
						<th class="py-3 pr-4 text-left font-medium text-muted"></th>
						{#each compareCols as c, i (c)}
							<th
								class={`px-4 py-3 text-center font-semibold ${i === 0 ? 'text-accent' : 'text-muted'}`}
								>{c}</th
							>
						{/each}
					</tr>
				</thead>
				<tbody>
					{#each compareRows as row (row.label)}
						<tr class="border-b border-border">
							<td class="py-3 pr-4 text-left">{row.label}</td>
							{#each row.cells as ok, i (i)}
								<td class={`px-4 py-3 text-center ${i === 0 ? 'bg-accent-soft/40' : ''}`}>
									{#if ok}
										<span class="inline-flex text-accent"
											><Icon icon={CheckmarkCircle02Icon} size={18} /></span
										>
									{:else}
										<span class="inline-flex text-muted/40"><Icon icon={Cancel01Icon} size={16} /></span
										>
									{/if}
								</td>
							{/each}
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
		<p class="mt-6 text-center text-xs text-muted">
			Comparison reflects deployment model; hosted platforms are excellent managed services — Ferrite
			trades managed convenience for ownership and control.
		</p>
	</div>
</section>

<!-- Stats band -->
<section class="border-t border-border">
	<div class="mx-auto grid max-w-6xl grid-cols-2 gap-8 px-6 py-16 lg:grid-cols-4" use:reveal>
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
	<div class="mx-auto max-w-6xl px-6 py-20" use:reveal>
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
	<div class="mx-auto max-w-6xl px-6 py-20" use:reveal>
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
	<div class="mx-auto max-w-6xl px-6 py-20" use:reveal>
		<div class="mx-auto max-w-2xl text-center">
			<h2 class="text-3xl font-semibold tracking-tight">Fixed monthly pricing, clear limits</h2>
			<p class="mt-3 text-muted">
				No metered surprises — each plan includes a set amount of storage, transcoding, and
				delivery. Self-host it free, or let us run it. 14-day free trial on cloud plans.
			</p>
		</div>
		<div class="mt-14 grid gap-6 sm:grid-cols-2 lg:grid-cols-4 lg:items-stretch">
			{#each tiers as t (t.name)}
				<div
					class={`flex flex-col rounded-xl border p-6 ${
						t.highlight ? 'border-accent bg-surface shadow-lg ring-1 ring-accent' : 'border-border bg-surface'
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
					<p class="mt-1 min-h-10 text-sm text-muted">{t.blurb}</p>
					<div class="mt-4 flex items-end gap-1">
						<span class="text-3xl font-semibold tracking-tight">{t.price}</span>
						{#if t.cadence}<span class="mb-1 text-sm text-muted">{t.cadence}</span>{/if}
					</div>

					{#if t.caps}
						<div class="mt-4 grid grid-cols-3 gap-2 rounded-lg border border-border bg-surface-2 p-3 text-center">
							{#each [['Storage', t.caps.storage], ['Transcode', t.caps.minutes], ['Delivery', t.caps.delivery]] as [label, val] (label)}
								<div>
									<p class="mono text-xs font-semibold">{val}</p>
									<p class="text-[10px] text-muted">{label}</p>
								</div>
							{/each}
						</div>
					{:else}
						<p class="mt-4 rounded-lg border border-border bg-surface-2 p-3 text-center text-xs text-muted">
							{t.capsNote}
						</p>
					{/if}

					<a
						href="/app"
						class={`mt-4 inline-flex items-center justify-center gap-2 rounded-lg px-4 py-2.5 text-sm font-medium transition-colors ${
							t.highlight
								? 'bg-accent text-accent-fg hover:opacity-90'
								: 'border border-border hover:bg-surface-2'
						}`}
					>
						{t.cta} <Icon icon={ArrowRight01Icon} size={15} />
					</a>
					<ul class="mt-6 flex flex-col gap-2.5 border-t border-border pt-6">
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
		<p class="mt-8 text-center text-xs text-muted">
			Exceeded a limit? Upgrade any time — no overage bill lands without warning. Prices
			illustrative.
		</p>
	</div>
</section>

<!-- FAQ -->
<section class="border-t border-border">
	<div class="mx-auto max-w-3xl px-6 py-20" use:reveal>
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
	<div class="mx-auto max-w-4xl px-6 py-20 text-center" use:reveal>
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
