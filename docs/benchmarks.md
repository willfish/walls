# Benchmarks

The core crate has Criterion benchmarks for hot paths that are easy to exercise
without live network access:

- local candidate selection and local source image listing
- download quota enforcement over deterministic temporary files
- Wallhaven cache path lookup for direct hits, legacy-extension scans, and misses
- source dispatch through provider descriptor classification

Run the full benchmark set with:

```bash
cargo bench -p walls-core --bench hot_paths
```

For a quick local smoke run that compiles the benchmark target and takes only a
small number of samples, use:

```bash
cargo bench -p walls-core --bench hot_paths -- --sample-size 10 --measurement-time 1
```

The fixtures are intentionally small and deterministic. File-system benchmarks
use temporary directories; the quota benchmark recreates files for each
iteration because enforcing quota deletes inputs. The Wallhaven benchmarks only
exercise cache-path lookup and do not call the Wallhaven API.
