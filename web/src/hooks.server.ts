import type { HandleServerError } from '@sveltejs/kit';

// Central server-side error handler. Logs the full error server-side and returns
// a safe, structured shape to the client — no stack traces leak to users.
export const handleError: HandleServerError = ({ error, event, status, message }) => {
	const errorId = crypto.randomUUID();
	console.error(`[server:${errorId}] ${event.request.method} ${event.url.pathname}`, error);

	return {
		message: status >= 500 ? 'An unexpected error occurred.' : message,
		errorId
	};
};
