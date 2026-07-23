# Ferrite web (SvelteKit / adapter-node) — build from repo root:
#   docker build -f web/Dockerfile --build-arg PUBLIC_API_URL=https://api.example.com .
FROM node:22-slim AS build
WORKDIR /app
RUN npm install -g pnpm@9
COPY web/package.json web/pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile
COPY web/ ./
# PUBLIC_* vars are baked into the client bundle at build time.
ARG PUBLIC_API_URL=http://localhost:8091
ENV PUBLIC_API_URL=$PUBLIC_API_URL
RUN pnpm build && pnpm prune --prod

FROM node:22-slim
WORKDIR /app
ENV NODE_ENV=production
ENV PORT=3000
COPY --from=build /app/build ./build
COPY --from=build /app/node_modules ./node_modules
COPY --from=build /app/package.json ./package.json
EXPOSE 3000
CMD ["node", "build"]
