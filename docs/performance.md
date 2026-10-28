# Local workbook timing notes

On 28 October 2026, an optimized build was checked with a synthetic XLSX containing 200,000 numeric rows in one column. The test used a CachyOS x86-64 environment with a Ryzen 5 5600, Hyprland, and WebKit software rendering. Each result is the median of three separate launches using the opt-in frame timing log.

| Build | Window frame | First content frame |
| --- | ---: | ---: |
| Before progressive scan | 211 ms | 386 ms |
| Progressive scan | 208 ms | 248 ms |

The first-content figure improved by about 36% in this sample. These are approximate in-app frame markers, not compositor presentation measurements. This small local comparison does not establish a performance target or measure peak memory. The first frame can precede completion of the background scan. Generate a similar synthetic file with `scripts/make_benchmark_workbook.py`, then follow the README's profiling instructions to measure another machine.
