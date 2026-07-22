// The dashboard is a client-rendered SPA: auth lives in localStorage, so
// server-rendering would flash the signed-out shell before hydration. Disable SSR.
export const ssr = false;
