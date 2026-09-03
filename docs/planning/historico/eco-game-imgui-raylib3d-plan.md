# Plano ECO — pacotes de jogo Ori (stack code-first)

> **Status:** plano ativo de **pacotes externos** (repos irmãos / novos).  
> **Não** reincorpora esses pacotes no monorepo `ori-lang` nem na Engine A.  
> **Baseline linguagem:** S3 `0.3.0` + inference B `0.3.1` + package `0.3.4` (FREEZE-1 / ABI-1).  
> **Criado:** 2026-07-13 · **Atualizado:** 2026-07-13 (rres, camada Ori, sqlite; futuro §17)  
> **Repos atuais:** `ori-game`, `ori-imgui`  
> **Repos novos (a criar):** `ori-raygui`, `ori-box2d`, `ori-jolt`, `ori-rres`, `ori-sqlite`  
> **Contrato nativo:** ABI C (`extern c`); artefatos em `lib/<triple>/`; preferir upstream C ou shim C sobre C++.

---

## 0. Objetivo e fora de escopo

### Decisão de produto (2026-07-13) — faixas e intenções

**Não** substituir a Ori Game Engine em desenvolvimento (`ori-game-engine`:
wgpu + Rapier + egui). Ela segue o produto 3D+editor.

Há **outra** intenção legítima: ecossistema de **pacotes Ori** em cima de C
(raylib, física, raygui, cimgui) e, **no futuro**, uma **segunda** engine
**code-first** (editor/UI) que reutilize esse motor raylib — sem tocar o host
Rust atual.

| Faixa | Repo / forma | Papel | Stack |
|-------|----------------|--------|--------|
| **Engine A (atual)** | `ori-game-engine` | 3D retro + editor Unity-like + Play Ori | Rust · wgpu · Rapier · egui |
| **Pacotes Ori (ECO)** | `ori-game`, `ori-imgui`, futuros `ori-box2d` / `ori-raygui` / … | Traduzir libs C → API Ori (`extern c` + wrappers) | C ABI · path/native_libs |
| **Engine B (futura, opcional)** | *ainda não existe* | Editor/UI em cima do motor raylib já “pronto” em Ori | Host GUI + link nos pacotes ECO |

**Física 2D/3D e raygui neste plano** = **bindings/pacotes para a linguagem
Ori**, não features do monorepo da Engine A.

### Escopo fechado do ecossistema (itens 1–4)

| # | Conteúdo | No plano |
|---|----------|----------|
| **1** | Stack nativa de jogo: raylib, raygui, imgui, Box2D, Jolt, raylib 3D draw | Trilhas **G, I, RG, B2, R, P3** |
| **2** | **rres** (pack de assets) | Trilha **RR** → `ori-rres` |
| **3** | Camada **Ori** em `ori-game`: tween/easings, scene stack, asset cache, save JSON | Trilha **O** (sem lib C nova) |
| **4** | **SQLite** (persistência genérica + saves) | Trilha **S** → `ori-sqlite` |
| **5** | Spine, net, compressão avançada, etc. | **Só futuro** — §17 (não implementar agora) |

| Pacote (alvo) | Lib upstream | Uso | Trilha |
|---------------|--------------|-----|--------|
| **ori-game** (`game` / raylib) | [raylib](https://github.com/raysan5/raylib) | Janela, draw 2D/3D, input, áudio + **módulos O** | G + R + **O** |
| **ori-imgui** (`imgui`) | [cimgui](https://github.com/cimgui/cimgui) / Dear ImGui | UI densa / tools | I |
| **ori-raygui** (`raygui`) | [raygui](https://github.com/raysan5/raygui) | UI imediata estilo raylib | RG |
| **ori-box2d** (`box2d`) | [Box2D](https://github.com/erincatto/box2d) (v3 C API) | Física 2D dinâmica | B2 |
| **ori-jolt** (`jolt`) | [Jolt](https://github.com/jrouwe/JoltPhysics) + C API | Física 3D (decisão fechada) | P3 |
| **ori-rres** (`rres`) | [rres](https://github.com/raysan5/rres) | Pack/unpack de recursos | **RR** |
| **ori-sqlite** (`sqlite`) | [SQLite](https://sqlite.org) amalgamation C | DB local / saves / inventário | **S** |

**Engine B = “só criar a interface gráfica?”** — **parcialmente sim**, se os
pacotes ECO estiverem maduros:

| Já “pronto” com pacotes bons | Ainda precisa construir na Engine B |
|------------------------------|-------------------------------------|
| Loop de frame, draw, input, áudio (raylib) | Projeto/cenas, serialização, asset pipeline |
| raygui / imgui no frame | Hierarchy, inspector, docks, undo, gizmos |
| Física via package | Wiring editor↔simulação, colliders editáveis |
| Gameplay em `.orl` | Play/stop, hot reload, packaging do jogo |

Ou seja: o **motor de jogo** fica mais barato; o **editor** continua um
produto (menor que Engine A se for simples, mas não é “só um form”).

**Fronteira com Engine A:** zero dependência obrigatória. Pacotes ECO podem
inspirar API `game.*` da Engine A, mas **não** viram runtime do host wgpu.

### Objetivo deste plano (faixa pacotes ECO / linguagem Ori)

1. Adaptar **ori-game** e **ori-imgui** à superfície S3 até check/compile/run estáveis no Linux.
2. **Traduzir** para Ori (FFI + wrappers + demos + `lib/<triple>/`):
   - **raygui** → `ori-raygui`
   - **Box2D** → `ori-box2d`
   - **Jolt** → `ori-jolt` (decisão fechada §8)
   - **rres** → `ori-rres`
   - **SQLite** → `ori-sqlite` (ECO genérico; demos de save com game)
3. Trilha **Raylib 3D draw** no ori-game (não é solver de física).
4. Camada **Ori** no ori-game: tween, scenes, assets, save JSON (**O**).
5. Manter fronteira com a Engine A (Rapier fica no host Rust).

### Fora de escopo **agora** (item 5 — ver §17)

| Item | Por quê |
|------|---------|
| Spine / DragonBones | Anim esquelética — sob demanda futura |
| Multiplayer / ENet / etc. | Netcode — futuro |
| Compressão avançada (basis, zip gordo) | Depois de rres |
| Substituir Engine A por raylib | Produto diferente |
| Ori Game Studio / Rapier no package | Fora desta faixa |
| Binding 1:1 completo de headers | Incremental |
| Marketplace / M4 | Adiados no monorepo |

### Princípios

1. **Pacotes externos** com `ori.pkg.toml` + `native_libs`.
2. **S3 first:** `module`, `import path = alias`, sem `func` em declaração, `list[T]`.
3. **FFI honest:** handles opacos / scalars; shim C quando struct-by-value.
4. **Smoke honesto:** stub headless para CI; raylib real para demos gráficos.
5. **Core fino:** bindings pesados em pacotes irmãos; `ori-game` = raylib + `game.*` Ori.
6. **stdlib first** para JSON/FS; C só quando o valor for claro (sqlite, rres, física).

---

## 1. Estado de partida (2026-07-13)

### ori-game

| Dimensão | Estado |
|----------|--------|
| Local | `~/Documentos/Projetos/ori-game` (~50 `.orl`, ~5k LOC) |
| Sintaxe | Pré-S3: 50× `namespace`, 185× `import as`, 288× `func`/`pub func`, 55× `list of` |
| `ori check` (0.3.x) | Falha imediata (`parse.namespace_removed`, …) |
| FFI | `raylib.orl` ~63 símbolos 2D/window/input/audio; **zero 3D** |
| Nativo Linux | `libraylib.a` stub ~9 KB; Windows tem `raylib.lib` real |
| Camadas Ori | L0 FFI → L1 wrappers → L2 core → L3 systems → L4 mechanics → demos |
| Package | `name = "ori_game"`, `ori_version = "0.2.0"`, `native_libs = ["raylib"]` |

### ori-imgui

| Dimensão | Estado |
|----------|--------|
| Local | `~/Documentos/Projetos/ori-imgui` (~100 LOC) |
| Alvo | **Já é Dear ImGui** via [cimgui](https://github.com/cimgui/cimgui) |
| FFI | 6 `ig*`; sem backend de render/input |
| Nativo | `lib/*` só `.gitkeep`; script Windows `tools/build_cimgui.ps1` |
| Package | `name = "imgui"`, `ori_version = "0.2.0"` |

### Ferramentas úteis já existentes

- `ori migrate-syntax` (mecânico: namespace→module, strip `func`, `import as`→`=`, `list of`→`[]`, …)
- `ori-game/tools/setup_raylib_linux.sh` (+ `--stub`)
- `ori-game/tools/smoke_linux.sh`
- `ori.mem.string_as_ptr` ainda no runtime (útil para títulos/labels C)

---

## 2. Arquitetura-alvo dos pacotes

```
ori-game/                          # repo irmão
├── ori.pkg.toml                   # name = "game" ou "ori_game" (decidir na Fase 0)
├── raylib/
│   ├── ffi.orl                    # extern c cru (2D + depois 3D)
│   └── (opcional) types.orl       # newtypes / Color / handles
├── native/
│   ├── raylib_shim.c/.h           # pack/unpack Vector3, Camera3D, Model*, …
│   └── CMakeLists.txt | build.sh
├── game/                          # wrappers + engine (S3)
├── lib/<rustc-triple>/            # libraylib.a + libori_raylib_shim.a
├── examples/
├── tests/
└── tools/setup_*.sh | smoke_*.sh

ori-imgui/
├── ori.pkg.toml                   # name = "imgui"
├── imgui/
│   ├── ffi.orl
│   └── ui.orl                     # wrappers idiomáticos
├── native/                        # cimgui + backend escolhido
├── lib/<triple>/
├── examples/
└── tools/build_cimgui.sh|.ps1
```

**Convenção de triple:** alinhar a `x86_64-unknown-linux-gnu` (e futuros Win/macOS) como no monorepo runtime — **não** `linux-x64` legado do imgui, salvo alias documentado.

**Decisão de nome (Fase 0):**

| Opção | `name` no toml | Import |
|-------|----------------|--------|
| A (recomendada) | `game` | `import game.app` |
| B (atual) | `ori_game` | depende do resolver de pacote |

Preferir **A** se o driver instalar sob o nome do manifesto; manter README explícito.

---

## 3. Trilha A — Adaptar ori-game a S3 + 2D linkável

### Fase G0 — Higiene e baseline — **DONE 2026-07-13**

- [x] Branch `s3-adapt` (repo `ori-game`).
- [x] Bump `ori_version` → `0.3.0`, package `0.2.0`.
- [x] Superfície S3 em lib: `module`, `import =`, `public`, `list[T]`, struct `{ }`.
- [x] Módulos `game.*` / `raylib` (sem `ori.game.*`).

### Fase G1 — Typecheck da lib — **DONE 2026-07-13**

- [x] `ori check` verde em raylib + color/shape/collision + `game/*` + `mechanics/*`.
- [x] Testes: shapes, engine, physics, inventory, save — todos passing.

### Fase G2 — Nativo raylib 2D — **DONE 2026-07-13**

- [x] `setup_raylib_linux.sh` builda raylib **5.5** real (~2.7 MB `.a`) ou `--stub`.
- [x] `ori compile` + run `app_smoke` / `hello_game` com raylib real (SystemLinker).
- [x] `simple_game` headless logic demo.

### Fase G3 — Smoke + docs + demos canônicos — **DONE 2026-07-13**

- [x] `tools/smoke_linux.sh` (check lib+canônicos, tests, compile+run com timeout em windowed).
- [x] README S3; CHANGELOG `[0.2.0]`.
- [x] Canônicos: `hello_game`, `app_smoke`, `simple_game`.
- [ ] Outros `examples/*` legados ainda pré-S3 (fora do smoke; migrar depois).

### Critérios de aceite G (ori-game 2D)

| Critério | Status |
|----------|--------|
| Sintaxe lib | ✅ check 37/37 no smoke |
| Testes | ✅ 8 tests / 5 files |
| Link | ✅ raylib 5.5 real + hello/smoke |
| Docs | ✅ README S3 |
| CI local | ✅ `smoke_linux.sh` green |

**Esforço total G0–G3:** concluído 2026-07-13.

---

## 4. Trilha I — Adaptar ori-imgui (Dear ImGui) — **DONE 2026-07-13 (MVP)**

> Repo: `~/Documentos/Projetos/ori-imgui` · package `imgui` · module `imgui.ui`  
> Backend: **GLFW + OpenGL3** (plan B2)

### Entregue

- [x] I0: S3 surface (`module`, `import =`, `public`, braced structs)
- [x] I1: vendored imgui + GLFW; host `ori_imgui_*` C API
- [x] I2: widgets begin/end/button/text/checkbox; `examples/demo.orl`
- [x] `tools/build_linux.sh` + `tools/smoke_linux.sh` green (window runs)

**Layout:** `libori_imgui.a` (core+backends+host) + `libglfw3.a` + `libsysdeps.a` (ld script).


## 5. Trilha C — Raylib 3D (no mesmo pacote ori-game)

### Ideia-chave

3D **não** é port de engine: é **mais símbolos** da mesma `libraylib` + **shim C** para structs por valor.

```
Ori (handles / scalars)
    ↓ extern c
ori_raylib_shim (C): Camera3D, Vector3, Model* pack/unpack
    ↓
raylib (C)
```

### Fase R0 — Design FFI 3D (1–2 dias, doc no repo game)

Definir política:

1. **Handles opacos `int`/`u64`** para `Model`, `Mesh`, `Shader`, `Material` (alocados no shim ou IDs de tabela).
2. **Scalars / arrays fixos** para `Vector3` (x,y,z), cores, floats de câmera.
3. **Não** depender de layout C de struct no Ori até a linguagem expor isso de forma estável.
4. Prefixo de símbolos shim: `ori_rl_*` para não colidir com `LoadModel` cru se ambos existirem.

Inventário MVP (fatia 1):

| Grupo | Símbolos-alvo |
|-------|----------------|
| Camera | `UpdateCamera` (mode), get/set position/target/up/fovy, `BeginMode3D`/`EndMode3D` |
| Primitivas | `DrawCube`, `DrawCubeWires`, `DrawSphere`, `DrawPlane`, `DrawGrid` |
| Model | `LoadModel`, `UnloadModel`, `DrawModel`, `DrawModelEx` |
| Util | `GetMouseRay` (opcional fatia 2), `GetRayCollision*` (fatia 2) |

Fora do MVP: gen mesh completo, materials avançados, shaders custom, skeletal anim, rlgl cru.

### Fase R1 — Shim C + binding Ori (3–5 dias) — **DONE 2026-07-13**

- [x] `native/ori_raylib_shim.c` implementando MVP (handles + scalars).
- [x] Build → `libori_raylib_shim.a` linkado **com** `libraylib.a` (`setup_raylib_linux.sh`).
- [x] `raylib.orl` com `extern c` dos `ori_rl_*` (2D + 3D).
- [x] Wrappers `game/draw3d.orl` + `game/camera3d.orl` (+ L1 2D completo).

### Fase R2 — Demo 3D (2–3 dias) — **DONE 2026-07-13**

- [x] `examples/hello_3d.orl`: janela, grid, cubo, sphere, nudge de câmera + `assets/cube.obj`.
- [ ] (Opcional) `examples/model_viewer.orl`: viewer dedicado glTF/OBJ.
- [x] Smoke: compile+run; stub 3D no-ops se headless (`tools/smoke_linux.sh`).

### Fase R3 — Expansão controlada — **DONE 2026-07-13**

- [x] Raycast / picking: `game.ray3d` + `examples/pick_3d.orl`
- [x] `DrawBoundingBox` L0/L1
- [x] Model diffuse texture + model animations L0/L1
- [ ] Lights / custom shaders — still sob demanda

### Critérios de aceite R (MVP 3D)

| Critério | Métrica |
|----------|---------|
| Demo | grid + cube + camera interativa |
| Link | um único link line (raylib + shim + ori-runtime) |
| API | documentada em `docs/api-3d.md` (pacote game) |
| Stub | CI sem GPU/X11 não quebra |

**Esforço R0–R2:** ~**1–2 semanas** após G2 estável.

---

## 6. Trilha RG — Traduzir raygui → `ori-raygui` — **DONE 2026-07-13**

> Repo: `~/Documentos/Projetos/ori-raygui` · package `raygui` · module `raygui.ui`

### Entregue

- [x] RG0: scaffold, `vendor/raygui.h`, `tools/build_linux.sh` → `libraygui.a`
- [x] RG1: shim `ori_gui_*` (button, label, checkbox, slider, group_box, status_bar, …)
- [x] RG2: `examples/hello_raygui.orl` + `tools/smoke_linux.sh` green
- [x] `native_libs = ["raygui", "raylib"]` (libraylib copiada/colocada ao lado)

**Smoke:** `ori-raygui/tools/smoke_linux.sh` — check + compile + run 3s.

---

## 7. Trilha B2 — Traduzir Box2D → `ori-box2d` — **DONE 2026-07-13 (MVP)**

> Repo: `~/Documentos/Projetos/ori-box2d` · package `box2d` · module `box2d.world`  
> Upstream: Box2D **v3.1.0**

### Entregue

- [x] B2-0: vendor + cmake build `libbox2d.a` + `libori_box2d_shim.a`
- [x] B2-1: world/body slots (`int`), static/dynamic box, step, pose getters
- [x] B2-2: headless `examples/boxes_fall.orl` prints **`fell`**; `tools/smoke_linux.sh` green

### Nota FFI (importante)

Neste MVP, **argumentos `float` de Ori→C chegavam como 0** no shim deste
pacote (reproduzido com logs). API pública usa **mili-unidades `int`**
(metros×1000, dt em microssegundos). Raylib floats no `ori-game` continuam OK —
investigar depois se for bug de codegen/`native_libs` multi-lib.

### Relação com `game.physics`

| Camada | Papel |
|--------|--------|
| `game.physics` | Helpers leves sem Box2D |
| `box2d` package | Solver real |

---

## 8. Física 3D — opções e recomendação (pacote Ori)

> Escopo: **traduzir uma lib de física 3D para Ori** (Engine B / jogos code-first).  
> **Fora:** trocar Rapier na Engine A.

### Comparativo (para pacote FFI)

| Lib | Linguagem | API C amigável? | Qualidade jogos | Custo de binding Ori | Notas |
|-----|-----------|-----------------|-----------------|----------------------|--------|
| **[Jolt Physics](https://github.com/jrouwe/JoltPhysics)** | C++ | **Sim via C wrappers** ([joltc](https://github.com/amerkoleci/joltc), [JoltC](https://github.com/SecondHalfGames/JoltC/), zig-gamedev) | **Excelente** (moderna, multi-thread, AAA/indie) | **Médio–alto** | Recomendação principal |
| **[Bullet 3](https://github.com/bulletphysics/bullet3)** | C++ | C-API legada / parcial | Boa, madura, um pouco “antiga” | **Médio** | Mais exemplos antigos; API C menos limpa |
| **ODE** | C | **Nativa C** | Datada | **Baixo–médio** | Fácil FFI, pouca manutenção moderna |
| **ReactPhysics3D** | C++ | Precisa shim | Boa / média | Médio | Menos ecossistema de bindings |
| **PhysX** | C++ | Complexa | Excelente | **Muito alto** | Pesada, vendor NVIDIA, packaging difícil |
| **Rapier3D** | Rust | Só com cdylib C export | Excelente | Médio (Rust toolchain no build do package) | **Já é a Engine A** — package Ori = duplicar stack; só se quiser *mesmo* solver nos dois mundos |
| **raylib** `GetRayCollision*` etc. | C | Já no raylib | **Só queries**, não dynamics full | Baixo (trilha R) | Picking/raycast; **não** substitui rigid body world |

### Decisão (2026-07-13) — **fechada**

| | |
|--|--|
| **Escolha** | **Jolt Physics** + C API (**joltc** ou **JoltC**, pin no spike P3-0) |
| **Pacote** | `ori-jolt` (`name = "jolt"`) |
| **Fallback** | **Bullet 3** só se o spike de packaging Jolt/C falhar (Linux/Windows) |
| **Não** | PhysX, ODE como solver ECO; Rapier no package (fica na Engine A) |
| **Queries** | raylib `GetRayCollision*` etc. na trilha R — complementar, não substitui Jolt |

| Prioridade | Escolha | Quando |
|------------|---------|--------|
| **1 — decidido** | **Jolt + C API** | Pacote `ori-jolt` para dinâmica 3D |
| **2 — fallback** | **Bullet 3** | Só se joltc/JoltC falhar no packaging |
| **3 — queries** | raylib collisions | Trilha R; combinar com Jolt depois |

**Por que Jolt (e não Bullet) para Ori packages**

1. Qualidade e performance atuais de jogos 3D (melhor “vida útil” do binding).  
2. C API de terceiros já usada por bindings (Zig, C#, etc.) — mesmo padrão que cimgui.  
3. Separação clara: Engine A = Rapier (Rust); ECO = Jolt (C ABI) — sem forçar usuário de package a ter Rust.  
4. Character controller / layers / multi-thread encaixam melhor em Engine B futura.

**Trade-off honesto:** Jolt **não** é C puro — o package precisa **compilar C++** (ou distribuir `.a` pré-built) + linkar a C API. Mesmo modelo mental de cimgui.

### Trilha P3 — `ori-jolt` (após R e/ou em paralelo a B2)

#### Fase P3-0 — Spike — **DONE 2026-07-13**

- [x] Escolha: **Jolt v5.2** vendored + **custom `ori_jolt_*`** (não joltc) — milli-units
- [x] Build → `lib/<triple>/libori_jolt_shim.a` (real merge libJolt **ou** Euler stub)
- [x] `extern c` smoke: create system, step, destroy
- [x] `docs/p3-jolt-spike.md` no package

#### Fase P3-1 — MVP binding — **DONE 2026-07-13**

| Capacidade | Superfície |
|------------|------------|
| System / world | create, step(dt µs), destroy, gravity |
| Bodies | box, sphere, capsule; static/dynamic |
| Transform | get position/velocity; quat ×1000; set pos/vel; impulse |
| Layers / broadphase | HelloWorld-style NON_MOVING/MOVING no shim C++ |
| Raycast | 1 hit + last_hit_* |

#### Fase P3-2 — Demo product — **DONE 2026-07-13**

- [x] `examples/boxes_fall.orl` + `impulse_test.orl` + `constraint_test.orl`
- [x] README: stub vs real Jolt; link com ori-game 3D documentado
- [x] Demo visual `demos/jolt_boxes_3d` (deps `jolt` + `ori_game`, stage multi-lib)
- [x] Constraints: fixed, distance, hinge

#### Critérios de aceite P3

| Critério | Métrica | Status |
|----------|---------|--------|
| Spike | system step sem crash | **OK** |
| MVP | ≥1 body dinâmico sob gravidade | **OK** (`fell`) |
| Demo | smoke product (impulse `moved`) | **OK** |
| Constraints | `constrained` example | **OK** |
| Visual | jolt_boxes_3d smoke | **OK** |

**Esforço P3-0–P3-2:** ~**4–7 semanas** (maior trilha nativa do plano).

### Alternativa se quiser adiar P3

1. Completar **R** (draw 3D + raycast raylib).  
2. Física 3D “de mentira” (kinematic + queries) nos demos.  
3. Abrir **P3** só quando Engine B ou um jogo 3D code-first exigir dynamics.

---

## 9. Trilha O — Camada Ori em `ori-game` (item 3) — **DONE 2026-07-13**

> **Sem lib C nova.** Módulos `.orl` no repo `ori-game`, S3, stdlib.

### Entregue

| Módulo | API |
|--------|-----|
| **`game.tween`** | linear, smoothstep, ease_*, lerp, `Tween` + update/value |
| **`game.scene`** | stack push/pop/replace/current/depth |
| **`game.assets`** | cache puro path→handle + remember/get/has/clear |
| **`game.asset_loader`** | LoadTexture/LoadSound/unload_all (raylib) |
| **`game.save`** | checkpoints + save_json/load_json/is_valid_json (fs+json) |

- [x] O0 tween + scene + testes headless  
- [x] O1 assets/loader + save FS + `examples/scene_menu.orl`  
- [x] O2 README + smoke + CHANGELOG  

**Smoke:** `./tools/smoke_linux.sh` inclui scene_menu.

---

## 10. Trilha RR — Traduzir rres → `ori-rres` — **DONE 2026-07-13 (MVP)**

> Repo: `~/Documentos/Projetos/ori-rres` · package `rres` · module `rres.pack`

### Entregue

- [x] ORPK pack (magic `ORPK`) + shim C; ids via `rresComputeCRC32` (vendored `rres.h`)
- [x] API: create/add_file/save/open/count/size_of/export
- [x] Demo `pack_roundtrip.orl` + `tools/smoke_linux.sh` → **ok**

**Nota:** load de arquivos `.rres` oficiais raysan5 = fatia futura; ORPK cobre bundles code-first.

---

## 11. Trilha S — Traduzir SQLite → `ori-sqlite` — **DONE 2026-07-13 (MVP)**

> Repo: `~/Documentos/Projetos/ori-sqlite` · package `sqlite` · module `sqlite.db`  
> Upstream: amalgamation **3.46.1** (`sqlite-amalgamation-3460100`)

### Entregue

- [x] `libsqlite3.a` + `libori_sqlite_shim.a`
- [x] API: open/close/is_open/exec/query_int/last_insert_rowid
- [x] Demo `kv_store.orl` + smoke → **ok**

### Relação com `game.save`

| | `game.save` (O) | `ori-sqlite` |
|--|-----------------|--------------|
| Formato | JSON arquivo | SQL |
| Uso | settings/checkpoints | inventário, quests, saves estruturados |

---

## 12. Ordem global recomendada (DAG)

```
G0–G1 S3 game ──► G2 raylib 2D ──► G3
         │              │
         ├──► O0 tween/scene (headless)
         │              ├──► O1 assets + save real
         │              ├──► RG  ori-raygui
         │              ├──► B2  ori-box2d
         │              ├──► RR  ori-rres
         │              ├──► R   raylib 3D draw
         │              │         └──► P3 ori-jolt
         │              └──► (demo S2 com game)
         │
I0–I2 imgui (B1 após G2)
S0–S2 ori-sqlite (paralelo após G1; não precisa GPU)
```

**Sequência de valor (recomendada):**

1. **G** — motor 2D usável  
2. **O** (tween/scene/save) + **RG** + **B2** — gameplay 2D completo  
3. **S** — persistência séria (paralelo possível cedo)  
4. **RR** — packs de assets  
5. **I** — tools densas  
6. **R** + **P3** — 3D draw + Jolt  

**Calendário agressivo (1 pessoa, itens 1–4):**

| Semana | Foco |
|--------|------|
| 1–2 | G0–G2 |
| 3 | G3 + O0–O1 |
| 4 | RG + início B2 ou S |
| 5–6 | B2 + O2 |
| 6–7 | S MVP + RR início |
| 7–8 | RR + I se prioridade |
| 8–9 | R |
| 10–14 | P3 Jolt |

---

## 13. Mapa de pacotes (repos)

| Repo | `name` toml | Deps nativas | Deps Ori |
|------|-------------|--------------|----------|
| `ori-game` | `game` / `ori_game` | `raylib` (+ shim 3D) | stdlib; módulos **O** |
| `ori-imgui` | `imgui` | cimgui + backend | stdlib; opcional game |
| `ori-raygui` | `raygui` | raygui + raylib | game |
| `ori-box2d` | `box2d` | box2d | stdlib; demo → game |
| `ori-jolt` | `jolt` | joltc/JoltC + Jolt | demo → game 3D |
| `ori-rres` | `rres` | rres | demo → game |
| `ori-sqlite` | `sqlite` | sqlite3 | stdlib; demo game opcional |

Triples: `x86_64-unknown-linux-gnu` primeiro; Win/macOS depois.

---

## 14. Skip / não planejar como package C

| Item | Motivo |
|------|--------|
| physac / Chipmunk | Box2D |
| raymath package | `ori.math` + wrappers |
| rini / rpng | stdlib / nicho |
| Rapier no ECO | Engine A |
| Lua / scripting paralelo | gameplay = Ori |

---

## 15. Riscos e mitigações

| Risco | Impacto | Mitigação |
|-------|---------|-----------|
| Struct-by-value raylib/Jolt | Binding quebrado | Shim C + handles |
| Stub raylib vs real | Falso positivo | Smoke separado |
| raygui × imgui | Trabalho dobrado | HUD vs tools |
| Box2D 2.x vs 3.x | Retrabalho | Pin 3.x |
| Jolt C API | Spike falha | P3-0; Bullet fallback |
| rres authoring vazio | Demo sem pack | Commit pack mínimo ou script de generate |
| SQLite amalgamation flags | Thread/size | Compile flags documentadas (SQLITE_OMIT_* só se preciso) |
| save JSON vs sqlite confusão | API duplicada | Docs trilha O vs S |
| Scope item 5 cedo | Atrasa 1–4 | §17 explícito |
| Confundir Engine A | Escopo | ECO ≠ host wgpu |

### Limitações da linguagem a aceitar

- Mutação por copy-return; callbacks nomeados; sem ECS real neste ciclo

---

## 16. Relação com o monorepo ori-lang

| Ação monorepo | Quando |
|---------------|--------|
| Bugs de `extern c` / link / `native_libs` | Fix living no compiler |
| Testes `tests/test_game_*.orl` órfãos | Remover ou mover para ori-game |
| Código dos pacotes dentro de ori-lang | **Não** |
| `BACKLOG.md` ECO-* | Ponteiros a este doc |

---

## 17. Futuro apenas (item 5) — **não** no escopo de implementação atual

Reabrir só com decisão explícita depois dos itens 1–4 entregues (ou MVP sólido).

| Tema | Exemplos | Nota |
|------|----------|------|
| **Animação esquelética 2D** | Spine runtime, DragonBones | Binding C médio; arte pipeline |
| **Rede / multiplayer** | ENet, GameNetworkingSockets, UDP raw | Fora de code-first single-player v1 |
| **Compressão / texturas avançadas** | Basis Universal, KTX2, zip completo | Depois de rres; zip fino opcional se rres não bastar |
| **Áudio AAA** | FMOD, OpenAL solto | Raylib áudio no v1 |
| **Shaders / rlgl grosso** | Efeitos custom | Depois de R estável |
| **Tiled full** | Parser TMX C | Preferir export JSON + Ori antes de C |
| **UI extra** | Nuklear, etc. | raygui + imgui bastam |

---

## 18. Checklist de abertura de implementação

1. [ ] Ordem base: **G → O + RG + B2 → S + RR → R → P3**; **I** conforme backend.  
2. [ ] Backend ImGui: raylib vs GLFW+GL.  
3. [x] Física 3D: **Jolt** (2026-07-13); Bullet só se spike falhar.  
4. [x] Escopo 1–4 no plano; item 5 só §17.  
5. [ ] Pin: raylib, raygui, Box2D 3.x, cimgui, joltc/JoltC, rres, sqlite amalgamation.  
6. [ ] Repos irmãos; monorepo só se bug de linguagem.

---

## 19. Resumo executivo de esforço

| Trilha | Conteúdo | Esforço |
|--------|----------|---------|
| **G** | S3 + raylib 2D + smoke | **1,5–3 sem** |
| **O** | tween, scene, assets, save JSON | **1–1,5 sem** |
| **I** | cimgui + backend + demo | **1,5–2,5 sem** |
| **RG** | raygui | **1–1,5 sem** |
| **B2** | Box2D | **2–3 sem** |
| **S** | SQLite | **2–3,5 sem** |
| **RR** | rres | **1,5–2,5 sem** |
| **R** | Raylib 3D draw | **1–2 sem** |
| **P3** | Jolt | **4–7 sem** |
| **Itens 1–4 sem 3D/Jolt** (G+O+RG+B2+S+RR+I) | | **~10–16 sem** |
| **+ R + P3** | | **~15–24 sem** (1 pessoa) |

**Primeiro valor:** hello 2D S3 + raylib (~2 sem).  
**Segundo:** O (scenes/save) + raygui ou Box2D.  
**Terceiro:** sqlite e/ou rres.  
**Quarto:** 3D draw + Jolt.

---

## 20. Próximo passo operacional

Quando autorizar implementação:

1. **G0+G1** em `ori-game` (migrate + check).  
2. **G2** nativo raylib.  
3. **O0** (tween/scene headless) em paralelo ao polish.  
4. Scaffold **ori-raygui**, **ori-box2d**, **ori-sqlite** (sqlite sem GPU).  
5. **RR** e **I** conforme prioridade de assets/tools.  
6. **R** → **P3** quando 3D for o foco.  
7. **Não** abrir §17 (item 5) sem decisão nova.
