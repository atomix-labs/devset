# `collection.toml`

<!-- reference: written by `just fix-docs` from `docs/src/schema/collection.json` -->

What a source says of itself, at its root.
[Publish a Collection](publish-a-collection.md) explains it.

Editors complete and check it from its schema, as [Schemas](schemas.md) says.

## `[collection]`

The `[collection]` table.

| Key           | Takes            | Meaning                                                       |
| ------------- | ---------------- | ------------------------------------------------------------- |
| `name`        | string, required | The name a target gives the source unless it chooses another. |
| `description` | string           | One line for humans.                                          |
