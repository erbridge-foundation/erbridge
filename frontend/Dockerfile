FROM node:lts-slim AS builder

ENV CI=true
RUN npm install -g pnpm

WORKDIR /app

COPY package.json pnpm-lock.yaml .npmrc ./
RUN pnpm install --frozen-lockfile

COPY . .

# Git-tag-derived version + short commit, computed outside the image (the build
# context has no .git/) and passed in by CI / `just docker-build-frontend`.
# vite.config.ts reads these envs and inlines them as PUBLIC_UI_VERSION /
# PUBLIC_GIT_COMMIT. With no build args they default empty → package.json sentinel
# version + "unknown" commit. See RELEASING.md.
ARG APP_VERSION=""
ARG GIT_COMMIT_SHA=""
ENV APP_VERSION=${APP_VERSION}
ENV GIT_COMMIT_SHA=${GIT_COMMIT_SHA}

RUN pnpm run build && pnpm prune --prod

FROM node:lts-slim

WORKDIR /app

COPY --from=builder /app/build ./build
COPY --from=builder /app/node_modules ./node_modules
COPY --from=builder /app/package.json ./package.json

USER node

EXPOSE 3000

CMD ["node", "build"]
