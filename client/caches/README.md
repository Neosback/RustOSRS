# Local cache directory

Development serves this directory as `/caches/` when `client/caches/caches.json` exists.

Expected shape:

```text
caches/
  caches.json
  <cache-name>/
    keys.json
    main_file_cache.dat2
    main_file_cache.idx255
    main_file_cache.idx0
    ...
```

Cache data is intentionally ignored by Git.

Instead of copying data here you can set:

```bash
RUSTOSRS_CACHE_DIR=/absolute/path/to/caches
```

or use a hosted cache with `REACT_APP_CACHE_BASE_URL` in `client/.env.local`.
