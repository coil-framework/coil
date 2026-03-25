# Harbor Shop Extraction Spec

- Source database snapshot: `fixtures/wordpress-events/source/db.sql.gz`
- Source uploads root: `fixtures/wordpress-events/source/uploads`
- Snapshot timezone: `Europe/London`
- Record extraction batches: users, media, pages, then events

The checked-in fixture package models the import contract used by `platform import run`.
