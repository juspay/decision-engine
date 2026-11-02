FROM node:20-alpine AS builder

WORKDIR /app

COPY package.json package-lock.json ./
RUN npm ci --include=dev

COPY . .
RUN VITE_DASHBOARD_BASE_PATH=/dashboard/ VITE_API_BASE_PATH=/decision-engine-api npm run build

# Runs as uid 101 and listens on 8080; nginx/nginx.conf matches that.
FROM nginxinc/nginx-unprivileged:alpine

COPY --from=builder --chown=root:root /app/dist/ /usr/share/nginx/html/dashboard/
