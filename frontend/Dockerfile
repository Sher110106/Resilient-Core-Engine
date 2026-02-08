# Stage 1: Build React app
FROM --platform=linux/amd64 node:18-alpine AS builder

WORKDIR /app

COPY package.json package-lock.json ./
RUN npm ci

COPY . .

# Empty string = use relative URLs, which nginx will proxy to the backend
ENV REACT_APP_API_URL=""
RUN npm run build

# Stage 2: Serve with nginx
FROM --platform=linux/amd64 nginx:alpine

# Copy custom nginx config
COPY nginx.conf /etc/nginx/conf.d/default.conf

# Copy built React app
COPY --from=builder /app/build /usr/share/nginx/html

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
