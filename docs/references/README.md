# docs/references — Reference Catalogue and Semantic Index

Canonical literature reference for the rust-ml Gomoku/AlphaZero project.

- `catalogue.md` — human-readable bibliography grouped by topic.
- `index.jsonl` — one JSON object per line; machine-searchable semantic index.

Search the index with ripgrep:

```bash
rg 'PUCT' docs/references/index.jsonl
rg 'root Dirichlet' docs/references/index.jsonl
```

To add an entry: verify metadata against arXiv/DOI/publisher, add it to
`generate.py`, run `python3 docs/references/generate.py`, then validate JSON
and link-check the resulting files.
