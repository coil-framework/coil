# Harbor Shop Cutover Runbook

1. Freeze legacy writes.
2. Run the final staged import package.
3. Apply target migrations and auth package changes.
4. Publish assets and verify storage policy resolution.
5. Warm caches and validate sample routes and user journeys.
6. Switch DNS and monitor rollback triggers for the observation window.
