# Sengoo

Sengoo is a self-developed compiled language focused on practical engineering outcomes:

- Python interoperability for gradual migration from existing ecosystems
- Fast full/incremental compile loops for day-to-day development
- Native execution path through an LLVM backend
- Optional non-invasive reflection with sidecar metadata

Sengoo is still in active development, but the CLI workflow is already usable for real local projects.

## Practical Demos (Developer-Oriented)

If you want business-style proof points instead of only synthetic microbenchmarks, run:

```bash
# Sengoo vs Python hot-path runtime demo
python bench/demos/hotpath-risk-scoring/run_demo.py

# Sengoo auto reflection vs C++ manual registry demo
python bench/demos/reflection-auto-vs-cpp/run_demo.py
```

Latest demo snapshots (measured on **February 16, 2026**):

- Hot-path demo report:
  `bench/demos/hotpath-risk-scoring/results/1771254169774-risk-scoring-demo.json`
- Reflection ergonomics demo report:
  `bench/demos/reflection-auto-vs-cpp/results/1771255074700-reflection-auto-vs-cpp.json`

| Demo | Sengoo | Python / C++ |
|---|---:|---:|
| Hot-path runtime avg (ms) | 25.23 | Python: 1285.13 |
| Hot-path speed ratio | 50.93x faster than Python | baseline |
| Reflection rule file LOC | 28 | C++: 55 |
| Manual registry entries | 0 | C++: 2 |
| Missing dynamic rules | 0 | C++: 1 |

## Why Sengoo

## 1) Hybrid Python Migration, Not Rewrite-Only Migration

Sengoo runtime exposes a Python interop layer (see `runtime/src/python.rs`) so teams can keep Python orchestration while moving hot paths to compiled native modules.

Interop benchmark snapshot (measured on **February 16, 2026**):
`bench/results/1771234431756-python-interop.json`

| Runner | Loop avg (ms) | Calls/s | vs Python native |
|---|---:|---:|---:|
| Python native | 0.965 | 5.18M | baseline |
| Sengoo Runtime (PythonInterop) | 0.665 | 7.52M | -31.14% |
| C++ (CPython C API) | 0.718 | 6.97M | -25.65% |
| Rust (PyO3) | 1.069 | 4.68M | +10.74% |

## 2) Fast Feedback Through Incremental Pipeline Reuse

Compiler pipeline focus:

- Build/run cache and module fingerprint invalidation
- AST-aware edit classification (`noop` / `impl_only` / `interface_change`)
- Workset-aware backend orchestration
- Optional daemon mode for persistent process workflows

Cross-language scenario matrix snapshot (measured on **February 16, 2026**):
`bench/results/1771185238357-scenario-matrix.json`

| Metric (avg) | Sengoo | C++ | Rust | Python |
|---|---:|---:|---:|---:|
| Full compile (ms) | 835.92 | 1669.41 | 972.98 | 67.48 |
| Incremental after edit (ms) | 33.71 | 1702.23 | 1088.19 | 65.52 |
| Incremental reduction (%) | 95.99% | -2.28% | -4.95% | 2.61% |

Advanced pipeline snapshot (real edits + 100k/1000k scale, averaged on **February 18, 2026** from two runs):
`bench/results/1771390773767-advanced-pipeline.json` + `bench/results/1771392747911-advanced-pipeline.json`

Real incremental scenarios (`after_avg_ms`, Sengoo):

| Scenario | After avg (ms) |
|---|---:|
| `loop_body_change` | 39.77 |
| `function_signature_change` | 43.81 |
| `add_new_function` | 36.50 |

100k LOC full pipeline (Sengoo):

| Stage | Avg (ms) |
|---|---:|
| Frontend (`compile_frontend_llvm_avg_ms`) | 153.87 |
| Codegen object (`codegen_obj_avg_ms`) | 90.61 |
| Link (`link_avg_ms`) | 173.05 |
| End-to-end (`e2e_avg_ms`) | 417.53 |

10k-1000k four-language e2e compile comparison (`Sengoo / C++ / Rust / Python`):

| LOC | Sengoo (ms) | C++ (ms) | Rust (ms) | Python (ms) |
|---|---:|---:|---:|---:|
| 10k | 372.28 | 693.01 | 2246.86 | 157.18 |
| 100k | 417.53 | 1074.84 | 6625.35 | 832.91 |
| 1000k | 1827.84 | 4883.70 | 54642.47 | 8283.46 |

Sengoo 1000k stage split:
- Frontend: `1589.02ms` (`86.93%`)
- Codegen object: `76.77ms` (`4.20%`)
- Link: `162.04ms` (`8.86%`)

## 3) Runtime-Class Performance Track

Scenario runtime p50 average (same matrix file `1771185238357`):

| Language | Runtime p50 avg (ms) |
|---|---:|
| Sengoo | 8.92 |
| C++ | 8.55 |
| Rust | 8.86 |
| Python | 45.14 |

Interpretation:

- Sengoo runtime behavior is currently in the same class as C++/Rust in this loop-heavy matrix profile.
- In these samples, Sengoo is significantly faster than Python runtime execution.

## 4) Non-Invasive Reflection (Auto by Default)

Reflection in Sengoo is designed for low baseline overhead with an auto mode:

- Default mode is `--reflect=auto`
- Auto mode enables reflection only when reflect imports are detected (`import reflect;` / `import std::reflect;`)
- Force enable with `--reflect` or `--reflect=on`
- Force disable with `--reflect=off`
- Metadata emitted to sidecar JSON (`*.sgreflect.json`)
- Typed runtime invocation (`call_i64`/`call_f64`/`call_bool`) with signature checks
- Native reflection binding path is used when available (fallback handler path is retained)

Reflection build example:

```bash
sgc build examples/09_method_call.sg -O 2
```

Fine-grained reflection selection:

```bash
sgc build examples/09_method_call.sg -O 2 --reflect=on \
  --reflect-module examples/09_method_call.sg \
  --reflect-symbol examples/09_method_call.sg::main
```

Runtime reflection usage example (Rust):

```rust
use sengoo_runtime::{ReflectValue, ReflectionRuntime};

let rt = ReflectionRuntime::new("target/release/app.sgreflect.json");
let symbols = rt.list_symbols("examples/09_method_call.sg")?;
println!("symbols = {}", symbols.len());

let value = rt.call_i64("examples/09_method_call.sg", "main", &[])?;
println!("result = {}", value);
```

Reflection overhead benchmark:

```bash
cargo run -p sgc -- bench reflection runtime --warmup 1 --iterations 5
python ./scripts/reflection-perf-gate.py --mode soft --sample bench/results/<latest-reflection-report>.json
```

Reflection benchmark cases:

- `disabled`: compile with reflection fully off (baseline path)
- `enabled-unused`: compile with `--reflect=on`, runtime reflection API not called
- `enabled-used`: compile with `--reflect=on`, perform runtime symbol listing and typed reflection invoke

Current gate defaults:

- `soft`: enabled-unused overhead <= `25%`, enabled-used overhead <= `45%`
- `hard`: enabled-unused overhead <= `15%`, enabled-used overhead <= `30%`
- disabled regression check compares against `bench/baseline.json` key `reflection/<suite>/disabled` when available

## Quick Start

```bash
cargo build --release
```

```bash
target/release/sgc run examples/01_hello.sg
```

```bash
target/release/sgc build examples/05_loop.sg -O 2
```

Useful commands:

```bash
# type check
sgc check <file.sg>

# compile and run
sgc run <file.sg> -O 1

# build native binary
sgc build <file.sg> -O 2

# force full rebuild
sgc build <file.sg> -O 2 --force-rebuild

# optional daemon mode
sgc daemon --addr 127.0.0.1:48765
```

## VS Code Extension

- Extension package location: `vscode-sengoo/`
- Current package version: `1.0.0`

## Benchmark Reproducibility

Benchmark suites are maintained in a separate repository:

- `https://github.com/Hyper66666/bench`

Common commands:

```bash
python ./bench/scenario_matrix_bench.py
python ./bench/advanced_pipeline_bench.py
python ./bench/python_interop_bench.py
python ./bench/bootstrap_generality_bench.py
```

Fairness profile used in advanced pipeline comparison:

- C++: precompiled header enabled
- Rust: cargo incremental enabled (`CARGO_INCREMENTAL=1`)

## Documentation

- Tutorial: `docs/sengoo-tutorial.html`
- Language features: `docs/language-features.md`
- Development guide: `docs/DEVELOPMENT_GUIDE.md`

## Repository Layout

```text
Sengoo/
|-- compiler/        # Frontend, type checker, HIR/MIR pipeline
|-- runtime/         # Runtime support, Python interop, reflection runtime API
|-- tools/
|   |-- sgc/         # Compiler CLI
|   |-- sgfmt/       # Formatter
|   `-- sglsp/       # Language server
|-- examples/        # Language examples
|-- docs/            # Tutorial and developer docs
`-- vscode-sengoo/   # VS Code extension
```

## Project Status

Current stage: early but fast-iterating.

Current focus:

- Frontend architecture optimization
- Stronger incremental consistency under real edits
- Better interop/reflection ergonomics
- Tooling and developer experience polish

Notes:

- All benchmark numbers above are local-machine measurements and should be treated as trend indicators.
- Use the benchmark repository and CI gates to verify performance on your own hardware.

---

# Sengoo锛堜腑鏂囩増锛?

Sengoo 鏄竴闂ㄨ嚜鐮旂紪璇戝瀷璇█锛岃仛鐒﹀疄闄呭伐绋嬭惤鍦帮細

- 寮哄寲 Python 浜掓搷浣滐紝鏀寔娓愯繘杩佺Щ
- 鎻愬崌鍏ㄩ噺/澧為噺缂栬瘧鍙嶉閫熷害锛岀缉鐭紑鍙戣凯浠ｅ懆鏈?
- 鍩轰簬 LLVM 鐢熸垚鍘熺敓鍙墽琛屼骇鐗?
- 鎻愪緵榛樿鑷姩鐨勯潪渚靛叆寮忓弽灏勶紙sidecar 鍏冩暟鎹級

椤圭洰浠嶅湪蹇€熻凯浠ｏ紝浣嗘湰鍦?CLI 寮€鍙戞祦绋嬪凡缁忓彲鐢ㄣ€?

## 瀹炵敤 Demo锛堥潰鍚戝紑鍙戣€咃級

濡傛灉浣犲笇鏈涚湅鍒颁笟鍔￠鏍肩殑鍙惤鍦拌瘉鏄庯紝鑰屼笉鏄粎鏈夊悎鎴愬井鍩哄噯锛屽彲鐩存帴杩愯锛?

```bash
# Sengoo vs Python 鐑偣鎬ц兘 Demo
python bench/demos/hotpath-risk-scoring/run_demo.py

# Sengoo 鑷姩鍙嶅皠 vs C++ 鎵嬪伐娉ㄥ唽 Demo
python bench/demos/reflection-auto-vs-cpp/run_demo.py
```

鏈€鏂板揩鐓э紙娴嬮噺鏃ユ湡锛?*2026-02-16**锛夛細

- 鐑偣 Demo 鎶ュ憡锛?
  `bench/demos/hotpath-risk-scoring/results/1771254169774-risk-scoring-demo.json`
- 鍙嶅皠宸ョ▼鎬?Demo 鎶ュ憡锛?
  `bench/demos/reflection-auto-vs-cpp/results/1771255074700-reflection-auto-vs-cpp.json`

| Demo | Sengoo | Python / C++ |
|---|---:|---:|
| 鐑偣璺緞杩愯鏃跺潎鍊?(ms) | 25.23 | Python: 1285.13 |
| 鐑偣璺緞閫熷害姣?| 姣?Python 蹇?50.93x | 鍩虹嚎 |
| 鍙嶅皠瑙勫垯鏂囦欢 LOC | 28 | C++: 55 |
| 鎵嬪伐娉ㄥ唽鏉＄洰鏁?| 0 | C++: 2 |
| 鍔ㄦ€佽鍒欑己澶辨暟 | 0 | C++: 1 |

## 涓轰粈涔堥€夋嫨 Sengoo

## 1) 娣峰悎寮?Python 杩佺Щ锛岃€岄潪涓€娆℃€ч噸鍐?

Sengoo 鍦ㄨ繍琛屾椂鎻愪緵 Python 浜掓搷浣滃眰锛堣 `runtime/src/python.rs`锛夛紝鏀寔鈥淧ython 缂栨帓 + Sengoo 鐑偣妯″潡鈥濈殑娣峰悎鏋舵瀯銆?

浜掓搷浣滃熀鍑嗗揩鐓э紙娴嬮噺鏃ユ湡锛?*2026-02-16**锛夛細
`bench/results/1771234431756-python-interop.json`

| 璺緞 | Loop 骞冲潎鑰楁椂 (ms) | 鍚炲悙 (Calls/s) | 鐩稿 Python 鍘熺敓 |
|---|---:|---:|---:|
| Python 鍘熺敓 | 0.965 | 5.18M | 鍩虹嚎 |
| Sengoo Runtime (PythonInterop) | 0.665 | 7.52M | -31.14% |
| C++ (CPython C API) | 0.718 | 6.97M | -25.65% |
| Rust (PyO3) | 1.069 | 4.68M | +10.74% |

## 2) 蹇€熷弽棣堢殑澧為噺缂栬瘧閾捐矾

缂栬瘧閾捐矾閲嶇偣锛?

- build/run 缂撳瓨涓庢ā鍧楁寚绾瑰け鏁堟満鍒?
- AST 鎰熺煡缂栬緫鍒嗙被锛坄noop` / `impl_only` / `interface_change`锛?
- workset 鎰熺煡鍚庣璋冨害
- 鍙€?daemon 甯搁┗妯″紡

璺ㄨ瑷€鍦烘櫙鐭╅樀锛堟祴閲忔棩鏈燂細**2026-02-16**锛夛細
`bench/results/1771185238357-scenario-matrix.json`

| 鎸囨爣锛堝钩鍧囷級 | Sengoo | C++ | Rust | Python |
|---|---:|---:|---:|---:|
| 鍏ㄩ噺缂栬瘧 (ms) | 835.92 | 1669.41 | 972.98 | 67.48 |
| 澧為噺缂栬緫鍚庣紪璇?(ms) | 33.71 | 1702.23 | 1088.19 | 65.52 |
| 澧為噺鏀剁泭 (%) | 95.99% | -2.28% | -4.95% | 2.61% |

楂樼骇娴佹按绾垮揩鐓э紙鐪熷疄缂栬緫 + 100k/1000k 瑙勬ā锛屾祴閲忔棩鏈燂細**2026-02-16**锛夛細
`bench/results/1771390773767-advanced-pipeline.json` + `bench/results/1771392747911-advanced-pipeline.json`

鐪熷疄澧為噺鍦烘櫙锛坄after_avg_ms`锛孲engoo锛夛細

| 鍦烘櫙 | 骞冲潎鑰楁椂 (ms) |
|---|---:|
| `loop_body_change` | 39.77 |
| `function_signature_change` | 43.81 |
| `add_new_function` | 36.50 |

100k LOC 鍏ㄦ祦绋嬶紙Sengoo锛夛細

| 闃舵 | 骞冲潎鑰楁椂 (ms) |
|---|---:|
| Frontend (`compile_frontend_llvm_avg_ms`) | 153.87 |
| Codegen object (`codegen_obj_avg_ms`) | 90.61 |
| Link (`link_avg_ms`) | 173.05 |
| End-to-end (`e2e_avg_ms`) | 417.53 |

10k-1000k 鍥涜瑷€ e2e 缂栬瘧瀵规瘮锛坄Sengoo / C++ / Rust / Python`锛夛細

| LOC | Sengoo (ms) | C++ (ms) | Rust (ms) | Python (ms) |
|---|---:|---:|---:|---:|
| 10k | 372.28 | 693.01 | 2246.86 | 157.18 |
| 100k | 417.53 | 1074.84 | 6625.35 | 832.91 |
| 1000k | 1827.84 | 4883.70 | 54642.47 | 8283.46 |

Sengoo 1000k 闃舵鍗犳瘮锛?
- Frontend: `1589.02ms`锛坄86.93%`锛?
- Codegen object: `76.77ms`锛坄4.20%`锛?
- Link: `162.04ms`锛坄8.86%`锛?

## 3) 杩愯鏃舵€ц兘绛夌骇

鍦烘櫙 runtime p50 骞冲潎锛堝悓涓€鐭╅樀鏂囦欢 `1771185238357`锛夛細

| 璇█ | Runtime p50 骞冲潎 (ms) |
|---|---:|
| Sengoo | 8.92 |
| C++ | 8.55 |
| Rust | 8.86 |
| Python | 45.14 |

瑙ｈ锛?

- 鍦ㄨ寰幆瀵嗛泦鍨嬬煩闃典腑锛孲engoo 涓?C++/Rust 澶勪簬鍚屼竴閲忕骇銆?
- 鍦ㄨ繖浜涙牱鏈噷锛孲engoo 杩愯鏃舵樉钁楀揩浜?Python 瑙ｉ噴鎵ц銆?

## 4) 闈炰镜鍏ュ紡鍙嶅皠锛堥粯璁よ嚜鍔級

Sengoo 鍙嶅皠鑳藉姏閲囩敤鈥滈粯璁よ嚜鍔?+ 鍙己鍒跺紑鍏斥€濇ā鍨嬶細

- 榛樿 `--reflect=auto`
- 妫€娴嬪埌鍙嶅皠瀵煎叆鏃惰嚜鍔ㄥ惎鐢紙`import reflect;` / `import std::reflect;`锛?
- 鏄惧紡寮哄埗寮€鍚細`--reflect` 鎴?`--reflect=on`
- 鏄惧紡寮哄埗鍏抽棴锛歚--reflect=off`
- 杈撳嚭 sidecar 鍏冩暟鎹紙`*.sgreflect.json`锛?
- 鎻愪緵绫诲瀷鍖栬皟鐢紙`call_i64` / `call_f64` / `call_bool`锛?
- 鍘熺敓鍙嶅皠缁戝畾璺緞鍙敤鏃朵紭鍏堜娇鐢紝涓嶅彲鐢ㄦ椂鍥為€€

鍙嶅皠鏋勫缓绀轰緥锛?

```bash
sgc build examples/09_method_call.sg -O 2
```

缁嗙矑搴︾瓫閫夌ず渚嬶細

```bash
sgc build examples/09_method_call.sg -O 2 --reflect=on \
  --reflect-module examples/09_method_call.sg \
  --reflect-symbol examples/09_method_call.sg::main
```

杩愯鏃惰皟鐢ㄧず渚嬶紙Rust锛夛細

```rust
use sengoo_runtime::{ReflectValue, ReflectionRuntime};

let rt = ReflectionRuntime::new("target/release/app.sgreflect.json");
let symbols = rt.list_symbols("examples/09_method_call.sg")?;
println!("symbols = {}", symbols.len());

let value = rt.call_i64("examples/09_method_call.sg", "main", &[])?;
println!("result = {}", value);
```

鍙嶅皠鎬ц兘闂ㄧ锛?

```bash
cargo run -p sgc -- bench reflection runtime --warmup 1 --iterations 5
python ./scripts/reflection-perf-gate.py --mode soft --sample bench/results/<latest-reflection-report>.json
```

## 蹇€熷紑濮?

```bash
cargo build --release
```

```bash
target/release/sgc run examples/01_hello.sg
```

```bash
target/release/sgc build examples/05_loop.sg -O 2
```

甯哥敤鍛戒护锛?

```bash
# 绫诲瀷妫€鏌?
sgc check <file.sg>

# 缂栬瘧骞惰繍琛?
sgc run <file.sg> -O 1

# 缂栬瘧涓哄師鐢熶簩杩涘埗
sgc build <file.sg> -O 2

# 寮哄埗鍏ㄩ噺閲嶅缓
sgc build <file.sg> -O 2 --force-rebuild

# 鍙€?daemon 妯″紡
sgc daemon --addr 127.0.0.1:48765
```

## VS Code 鎻掍欢

- 鎻掍欢鐩綍锛歚vscode-sengoo/`
- 褰撳墠鎵撳寘鐗堟湰锛歚1.0.0`

## 鍩哄噯澶嶇幇

鍩哄噯濂椾欢缁存姢鍦ㄧ嫭绔嬩粨搴擄細

- `https://github.com/Hyper66666/bench`

甯哥敤鍛戒护锛?

```bash
python ./bench/scenario_matrix_bench.py
python ./bench/advanced_pipeline_bench.py
python ./bench/python_interop_bench.py
python ./bench/bootstrap_generality_bench.py
```

楂樼骇娴佹按绾垮叕骞虫€ч厤缃細

- C++锛氬惎鐢ㄩ缂栬瘧澶达紙PCH锛?
- Rust锛氬惎鐢?cargo incremental锛坄CARGO_INCREMENTAL=1`锛?

## 鏂囨。鍏ュ彛

- 鏁欑▼锛歚docs/sengoo-tutorial.html`
- 璇█鐗规€э細`docs/language-features.md`
- 寮€鍙戞墜鍐岋細`docs/DEVELOPMENT_GUIDE.md`

## 浠撳簱缁撴瀯

```text
Sengoo/
|-- compiler/        # 鍓嶇銆佺被鍨嬫鏌ャ€丠IR/MIR 娴佹按绾?
|-- runtime/         # 杩愯鏃舵敮鎸併€丳ython 浜掓搷浣溿€佸弽灏勮繍琛屾椂 API
|-- tools/
|   |-- sgc/         # 缂栬瘧鍣?CLI
|   |-- sgfmt/       # 鏍煎紡鍖栧伐鍏?
|   `-- sglsp/       # 璇█鏈嶅姟鍣?
|-- examples/        # 璇█绀轰緥
|-- docs/            # 鏁欑▼涓庡紑鍙戞枃妗?
`-- vscode-sengoo/   # VS Code 鎵╁睍
```

## 椤圭洰鐘舵€?

褰撳墠闃舵锛氭棭鏈燂紝浣嗗湪楂橀€熻凯浠ｃ€?

褰撳墠閲嶇偣锛?

- 鍓嶇鏋舵瀯浼樺寲
- 鐪熷疄缂栬緫涓嬫洿寮虹殑涓€鑷存€у閲忕紪璇?
- 浜掓搷浣滀笌鍙嶅皠浣撻獙鎸佺画鎵撶（
- 宸ュ叿閾句笌寮€鍙戣€呬綋楠屽畬鍠?

璇存槑锛?

- 涓婅堪鍩哄噯鍧囦负鏈満娴嬮噺鍊硷紝搴斾綔涓鸿秼鍔夸俊鍙疯€岄潪缁濆缁撹銆?
- 璇风粨鍚?bench 浠撳簱涓?CI gate 鍦ㄤ綘鐨勭‖浠剁幆澧冧笂澶嶉獙銆?

