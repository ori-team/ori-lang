# Graphics microbench suite (GFX-BENCH-1)

Official CPU software-rendering workload for the Ori language, following
[`docs/planning/ORI_GRAPHICS_LANGUAGE_EVOLUTION.md`](../../../docs/planning/ORI_GRAPHICS_LANGUAGE_EVOLUTION.md).
The renderer-from-scratch study path doubles as the performance workload:
optimizations must show wins here, not on synthetic loops.

## Kernels

| ID | Kernel | Work | Metric |
|----|--------|------|--------|
| GFX-BENCH-01 | `gfx01_fill` | fill 640×480 RGBA with one color | pixels/s |
| GFX-BENCH-02 | `gfx02_gradient` | x/y gradient (int→float, float ops, nested loops) | pixels/s |
| GFX-BENCH-03 | `gfx03_line` | 512 Bresenham lines (branching, scattered writes) | lines/s |
| GFX-BENCH-04 | `gfx04_triangle` | 256 triangles via edge functions | triangles/s |
| GFX-BENCH-05 | `gfx05_zbuffer` | 256 z-buffered triangles (two buffers) | triangles/s |
| GFX-BENCH-06 | `gfx06_vertex` | 2k `Vec4` × `Mat4` transforms | vertices/s |

Each kernel prints a deterministic canary (last pixel or fill count) so a
regression that changes output is caught without parsing timing noise.

## Run

```bash
# from repo root
ORI_BIN=compiler/target/debug/ori SAMPLES=3 ./tools/bench/graphics/run_graphics_bench.sh
```

If `ori` is on `PATH`, `ORI_BIN` is optional. The runner:

1. compiles each kernel AOT (`ori compile`);
2. runs each kernel `SAMPLES` times, recording wall time per run
   (Python `time.perf_counter` around the subprocess);
3. writes `results/report_<timestamp>.md` with medians, derived throughput,
   and compile times.

## Metrics tracked

- total wall time, pixels/s, triangles/s, vertices/s (per kernel);
- allocations and peak memory: add `ORI_DUMP_ARC=1` runs when measuring ARC;
- AOT vs JIT: compare `ori compile` + run against `ori run`;
- `ORI_OPT=none|default|aggressive`: rerun with each to measure the mid-end.

## Fairness & current limitations

- Process wall time includes startup + one print (all kernels identical shape).
- Framebuffers are `list[int]` until GFX-BUFFER-1 ships a contiguous
  `buffer[T]`; the kernels will switch without changing the algorithm.
- The depth buffer uses fixed-point ints — standard in software renderers —
  because the native runtime's `list[float]` read path is untested (a known
  pre-existing gap; see `gfx05` comment).
- Bitwise operators (`& | ^ ~ << >>`) landed with GFX-BITWISE-1; RGBA packing
  in `gfx02_gradient` uses `(r << 16) | (g << 8) | b`.

## Comparison languages

The polyglot runner (`tools/bench/polyglot/`) covers scalar loops across C,
Rust, Go, Nim, etc. Porting these kernels to C/Rust is the natural next step
for graphics-specific cross-language numbers (P0.4 of the evolution plan).
