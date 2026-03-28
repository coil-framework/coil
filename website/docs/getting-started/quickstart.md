---
title: Quickstart
---

Davenda is easiest to understand by running the demos.

## Prerequisites

- Rust toolchain
- Docker with Compose
- Node.js 20+ for the docs site

## Run Shoppr

```bash
cd apps/shoppr
cp .env.example .env
docker compose -f docker-compose.yml -f docker-compose.repo.yml up --build
```

Open:

- `http://uk.127.0.0.1.nip.io:8080/`
- `http://fr.127.0.0.1.nip.io:8080/`
- `http://pl.127.0.0.1.nip.io:8080/`
- `http://localhost:8080/__dev`

Shoppr is the reference path for learning Davenda through ecommerce.

## Run Gitly

```bash
cd apps/gitly
cp .env.example .env
docker compose up --build
```

Gitly exists to prove the platform is not restricted to commerce.

## Run The Public Docs

```bash
cd website
npm install
npm run start
```

## What To Learn First

- customer-app composition
- HTML-first rendering
- multi-site and multi-locale routing
- linked Rust backends
- WASM extension boundaries
- operations and deployment
