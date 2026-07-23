// Scroll-reveal: fades/slides an element in the first time it enters the
// viewport. No-op under reduced-motion or without IntersectionObserver.
export function reveal(node: HTMLElement, delay = 0) {
	const reduce = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
	if (reduce || typeof IntersectionObserver === 'undefined') return;

	node.classList.add('reveal');
	if (delay) node.style.transitionDelay = `${delay}ms`;

	const obs = new IntersectionObserver(
		(entries, o) => {
			for (const e of entries) {
				if (e.isIntersecting) {
					node.classList.add('reveal-in');
					o.unobserve(e.target);
				}
			}
		},
		{ threshold: 0.1, rootMargin: '0px 0px -8% 0px' }
	);
	obs.observe(node);
	return { destroy: () => obs.disconnect() };
}
