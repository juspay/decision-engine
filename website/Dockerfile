FROM node:20-alpine AS builder

WORKDIR /app

COPY package.json package-lock.json ./
RUN npm ci --include=dev

COPY . .
RUN VITE_DASHBOARD_BASE_PATH=/dashboard/ VITE_API_BASE_PATH=/decision-engine-api npm run build

FROM nginx:alpine

COPY --from=builder /app/dist/ /usr/share/nginx/html/dashboard/
