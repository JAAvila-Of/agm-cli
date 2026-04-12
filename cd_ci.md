# Plan: CI/CD refactor y automatización del checklist de distribución de `agm-cli`

> Documento de seguimiento de `cli_install.md`. Mientras aquel definió **qué** se distribuye y **cómo** se empaqueta, éste define **cómo se construye, prueba, smokea, auto-actualiza y despliega** eso de forma automática, en 5 pipelines separados. No toca el modelo de distribución; sólo su automatización.

## 1. Resumen ejecutivo

`cli_install.md` reemplazó todo el stack WiX/.pkg/Pillow por un modelo estándar de pre-built archives + scripts de instalación + `cargo install`. Ese plan se ejecutó: `azure-pipelines-deploy.yml` hoy corre un stage `Validate` + un stage `BuildBinaries` con matrix de 5 targets + `PublishCrate` + `GitHubRelease`, y existen `scripts/install.sh` e `scripts/install.ps1`. La verificación de que ese refactor funcionó es hoy un **checklist manual** (§12 de `cli_install.md`): descargar cada archivo, verificar checksum, extraer, correr `./agm --version`, repetir en 5 plataformas.

Este refactor automatiza ese checklist en su mayor parte, **separando el pipeline monolítico actual en cinco pipelines con responsabilidades claras** y **acotando el alcance de targets a Windows + macOS** (Linux queda cubierto por `cargo install agm-cli`, lo cual es aceptable porque el usuario actual es 100% audiencia Rust-nativa).

La arquitectura objetivo es:

1. `azure-pipelines-test.yml` — lint + tests + static checks (shellcheck, PSScriptAnalyzer, guardas `grep` sobre patrones prohibidos, opcionalmente `yamllint`).
2. `azure-pipelines-build.yml` — matrix build de **3 targets** (no 5), publica pipeline artifacts.
3. `azure-pipelines-smoke.yml` — consume los artifacts del build y ejecuta `install.sh` / `install.ps1` *localmente* contra un HTTP server que sirve los artifacts, overrideando `AGM_BASE_URL`. Verifica checksums y corre `./agm --version`.
4. `azure-pipelines-update.yml` — instala un release previo, corre `agm update`, verifica que llega al release actual. Scheduled + post-release.
5. `azure-pipelines-deploy.yml` — sólo `cargo publish` + `GitHubRelease@1`, consumiendo artifacts del build. Gate en smoke.

El beneficio neto es eliminar la ventana entre "tag" y "confianza en el release" de varias horas de verificación humana a unos minutos de pipeline, e inmunizar a futuros refactors de los scripts de instalación contra regresiones silenciosas.

## 2. Alcance y no-objetivos

### 2.1 En alcance

- **Renombrar** `azure-pipelines-build.yml` existente (lint + tests) → `azure-pipelines-test.yml`.
- **Crear nuevo** `azure-pipelines-build.yml` con el stage `BuildBinaries` actual extraído de `azure-pipelines-deploy.yml` y reducido a 3 targets.
- **Adelgazar** `azure-pipelines-deploy.yml` para que sólo contenga `Validate` + `PublishCrate` + `GitHubRelease`, consumiendo artifacts del build pipeline vía `resources.pipelines`.
- **Crear** `azure-pipelines-smoke.yml` que corre `install.sh` / `install.ps1` contra artifacts locales.
- **Crear** `azure-pipelines-update.yml` que ejercita `agm update` contra un release real.
- **Refactorizar** `scripts/install.sh` e `scripts/install.ps1` para soportar override `AGM_BASE_URL` sin romper el comportamiento default.
- **Reapuntar** `install.sh` Linux → mensaje "use `cargo install agm-cli`" + exit 0 (no error).
- **Añadir static checks**: `shellcheck`, `PSScriptAnalyzer`, grep-guard de patrones prohibidos, y opcionalmente `yamllint`.
- **Actualizar** `README.md` y `docs/releasing.md` reflejando la matriz reducida (W+M).
- **Phased rollout** Phase A (pre-v1.1.0) y Phase B (post-v1.1.0) para resolver el problema chicken-and-egg del primer release.
- **Añadir** `cd_ci.md` al allowlist de la grep-guard (no puede hacerla fallar a sí mismo por autorreferencia).

### 2.2 Fuera de alcance (tracks de seguimiento)

- **Targets Linux** (`x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`): deferidos. Rationale en §3.2. Añadirlos después es mecánico: una entrada más en el matrix del build + una entrada más en el smoke.
- **Homebrew tap**, **winget manifest**, **Scoop bucket**, **Chocolatey package**: ninguno.
- **Code signing** (Authenticode, Apple Developer ID + notarytool): ninguno.
- **Shell completions + man pages** (`clap_complete`, `clap_mangen`): trabajo del lado del CLI, no del pipeline.
- **Migración de CI a GitHub Actions**: el stack sigue siendo Azure Pipelines.
- **Cambios al crate `agm` en sí**, excepto una muy puntual posible adición de un test unitario que valide que el asset-name que `self_update` construye es compatible con el esquema de nombres del release. Se discute en §10 como Task A7 (opcional).
- **Reestructuración del artifact de crates.io**: `cargo publish` no cambia.
- **GitHub Release marcado como draft vs. published**: queda como está (`isDraft: true` durante Phase A, a criterio del usuario cambiar a `false` en Phase B).

## 3. Decisiones arquitectónicas

### 3.1 ¿Por qué 5 pipelines en vez del monolítico actual?

El `azure-pipelines-deploy.yml` de hoy acopla 4 responsabilidades dispares:

- Validar que la tag coincida con `Cargo.toml`.
- Compilar binarios (5× matrix).
- Publicar a crates.io.
- Publicar GitHub Release.

**Consecuencias del acoplamiento actual**:

1. No se puede **recompilar** un release sin re-tagear. Si un build falla por flakiness de la red, hay que inventar tags `v1.1.0-rc.N`.
2. No se puede **reintentar el deploy** sin rebuildear (y vice versa).
3. No se puede **ejercitar el pipeline sin release real**: cualquier ejecución termina publicando (o intentándolo) a crates.io.
4. No hay **gate de integración** entre "el binario compila" y "el binario instalable por usuarios finales funciona".
5. **Scheduled update tests** son imposibles: el pipeline solo dispara por tag.

El split en 5 pipelines resuelve cada punto mapeando cada responsabilidad a un pipeline con su propio trigger y su propio set de artifacts. El patrón de `resources.pipelines` en Azure Pipelines permite cadenas (build → smoke → deploy) sin re-ejecución.

**Alternativa descartada**: mantener un solo pipeline con stages skippables via parameters. Más compleja de mantener, peor separación de logs, no resuelve "probar install scripts sin release público".

### 3.2 ¿Por qué 3 targets y no 5?

Decisión del usuario: la audiencia actual de `agm-cli` es 100% desarrolladores Rust, para quienes `cargo install agm-cli` es indistinguible en UX de un one-liner. `cli_install.md` §3.2 ya marcó `aarch64-unknown-linux-gnu` como "puede diferirse". Este plan extiende esa deferral a **todos** los targets Linux.

**Ganancias concretas al cortar Linux**:

- **Cero** uso de `cross` → cero Docker en el runner → cero dolor con volumes en Windows host en caso de que algún contributor quiera correr el pipeline local.
- **Cero** `gcc-aarch64-linux-gnu`.
- **Cero** riesgo de flakiness por `qemu-user` o binfmt.
- §11.3 de `cli_install.md` (análisis de riesgo cross-compile aarch64-linux) desaparece completamente.
- Build matrix de `BuildBinaries` se reduce de ~20 min (serializado por el aarch64-linux más lento) a ~8-12 min (el lento ahora es macOS aarch64 compile + tar).
- Smoke pipeline se reduce de 5 a 3 jobs → duración dividida por ~1.66×.

**Coste**: un usuario Linux sin toolchain de Rust se encuentra con un error del one-liner. **Mitigación**: `install.sh` detecta Linux y muestra el mensaje amigable `"Linux binaries are not published. Install via: cargo install agm-cli"` + `exit 0`. No es un error del usuario, es una redirección explícita.

**Extensión futura**: añadir Linux es mecánicamente una entrada más en la matrix del build, una entrada más en la matrix del smoke, y un `case "$uname_s" in Linux)` que deja de redirigir. No requiere ningún cambio arquitectónico.

### 3.3 ¿Por qué `AGM_BASE_URL` como env var override?

Los scripts de hoy hard-codean `https://github.com/JAAvila-Of/agm-cli/releases/download/`. Para que smoke.yml pueda ejercitar los scripts *antes* de que exista un release público, hay tres caminos:

1. **Patch del script con `sed` en el pipeline**: feo, fácil de desincronizar, oculta el path de código que corre un usuario real.
2. **Publicar un draft release efímero**: requiere estar logueado contra GitHub, es un side-effect visible externamente, y complica el cleanup.
3. **Env var `AGM_BASE_URL`**: el script sigue siendo el mismo binario que el usuario descarga; el pipeline lo ejecuta con un `AGM_BASE_URL=http://localhost:8000/v1.1.0` y punto. El default preserva comportamiento.

**Decisión: Opción 3**. Es estándar (Rustup usa `RUSTUP_DIST_SERVER`, Nix usa `NIX_BINARY_CACHES`), es trivial de implementar (~5 líneas en cada script), y el *mismo código* corre en producción y en el smoke.

**Nombre** `AGM_BASE_URL`: paralelo a `AGM_VERSION`, `AGM_INSTALL_DIR` que ya existen.

**Semántica**: si `AGM_BASE_URL` está seteado, el script lo usa como base reemplazando `https://github.com/${REPO}/releases/download/${VERSION_TAG}`. Es decir, el archivo a descargar se construye como `${AGM_BASE_URL}/${ARCHIVE}`. Esto permite tanto apuntar a un mirror corporativo (futuro) como a un `http://localhost:8000/v1.1.0` en el smoke.

### 3.4 ¿Por qué `miniserve` y no `python3 -m http.server`?

Opciones para servir los artifacts localmente en el runner:

| Opción | Pro | Contra |
|---|---|---|
| `python3 -m http.server 8000` | Universal, 1 línea | Python disponibilidad varía (windows-latest sí, pero versión puede variar); bloquea el shell salvo `&`/`Start-Job` |
| `miniserve` (Rust) | Single binary cross-platform; ya tenemos Rust toolchain; `cargo install miniserve --locked` | +~1 min de compilación si no se cachea |
| `npx http-server` | Rápido si Node está | Requiere Node, no garantizado |
| PowerShell `HttpListener` | Sin deps externas (Windows) | Sólo Windows; código más verboso |

**Decisión: `miniserve`**. Razones:

- Ya tenemos Rust toolchain instalado en el job anyway (para `cargo` invocations previas o para los install hints).
- `cargo install miniserve --locked` es reproducible y podemos cachearlo vía `Cache@2` task si termina siendo pesado (~60s fresh).
- Un solo flag (`miniserve --port 8000 ./dist`) sirve todo el árbol sin configuración.
- Cross-platform: corre igual en windows-latest y macOS-latest.
- Flush de logs limpio para debugging.

**Backup**: si `miniserve` da problemas, fallback a `python3 -m http.server 8000` en background (`python3 -m http.server 8000 &` en bash; `Start-Process python3 ...` en PowerShell). La interfaz de red es idéntica, sólo cambia el proceso-launcher.

### 3.5 ¿Por qué gatear deploy con smoke?

Sin gate:

- Tag `v1.1.0` → Validate → BuildBinaries → PublishCrate → GitHubRelease.
- Si los archivos se publican pero los install scripts fallan contra ellos, el usuario es el primero en enterarse. `cargo yank` es la única salida y deja tracks en crates.io.

Con gate:

- Tag `v1.1.0` → Validate → BuildBinaries → **smoke (contra build artifacts)** → PublishCrate → GitHubRelease.
- Si smoke falla, crates.io no se tocó, GitHub Release no se creó. El tag queda en el repo pero no hay contaminación externa.
- Rollforward: pushear un fix, retagear `v1.1.1`, todo el pipeline corre de nuevo limpio.

**Coste**: +~5 min de pipeline tiempo total por release. Aceptable — el gate previene clases enteras de incidentes post-release que son caros de rectificar.

**Riesgo**: smoke tiene bugs propios y bloquea releases genuinos. **Mitigación**: smoke debe ser **determinista** (no red externa, usa `miniserve` local, no golpea GitHub API), y debe tener **logs verbosos** para diagnóstico. Sección §12 enumera los riesgos específicos.

### 3.6 ¿Por qué Phase A + Phase B?

Problema: el pipeline update.yml necesita un release *previo* para probar `agm update`. El primer release post-refactor (`v1.1.0`) no tiene previo. Lo mismo aplica al gate smoke → deploy: el primer release no tiene aún un `install.sh` probado contra artifacts *de ese release*, porque el smoke se corre con los artifacts *del build pipeline*, no de GitHub Releases.

**Phase A** libera `v1.1.0` con el pipeline nuevo pero sin los componentes que dependen del release previo:

- Rename test, split build/deploy, add `AGM_BASE_URL` override, add static checks, **omit smoke gate on deploy**, **omit update pipeline**. La verificación del primer release es manual (§12 del `cli_install.md`).
- Libera `v1.1.0`. Desde este momento existe un release previo contra el cual probar.

**Phase B** añade smoke.yml con gating y update.yml. Se valida con `v1.1.1` (una patch release bootstrap, puede ser un solo commit trivial como fix de changelog).

**Alternativa descartada**: diseñar smoke.yml de forma que en el primer run detecte "no hay previo" y pase en skip. Funciona para update.yml pero **no** para smoke.yml (el smoke debería pasar contra build artifacts de *este* build, lo cual es posible pero requiere que deploy.yml de Phase A ya consuma smoke como gate — cadena de cambios demasiado grande para un solo PR). Phase A/B es más incremental y permite descubrir bugs de cada componente por separado.

## 4. Arquitectura propuesta del CI/CD

```
                       ┌────────────────────────────┐
                       │   Developer pushes / PR    │
                       └────────────┬───────────────┘
                                    │
                                    ▼
                       ┌────────────────────────────┐
                       │  azure-pipelines-test.yml  │
                       │  ──────────────────────    │
                       │  • fmt                     │
                       │  • clippy                  │
                       │  • cargo test Windows      │
                       │  • cargo test macOS        │
                       │  • shellcheck install.sh   │
                       │  • PSScriptAnalyzer .ps1   │
                       │  • grep-guard banned pats  │
                       │  • yamllint pipelines      │
                       └────────────┬───────────────┘
                                    │
                                 merge
                                    │
                                    ▼
                       ┌────────────────────────────┐
                       │ azure-pipelines-build.yml  │
                       │  ──────────────────────    │
                       │  Trigger: push main, tag,  │
                       │           manual           │
                       │                            │
                       │  Matrix (3 targets):       │
                       │   • windows x86_64 .zip    │
                       │   • macOS x86_64 .tar.gz   │
                       │   • macOS aarch64 .tar.gz  │
                       │                            │
                       │  Pipeline artifacts:       │
                       │   release-<target> × 3     │
                       └────────────┬───────────────┘
                                    │
                    (resources.pipelines trigger)
                                    │
                                    ▼
                       ┌────────────────────────────┐
                       │ azure-pipelines-smoke.yml  │
                       │  ──────────────────────    │
                       │  Consumes build artifacts. │
                       │  On windows-latest:        │
                       │   • serve dist via         │
                       │     miniserve              │
                       │   • run install.ps1 with   │
                       │     AGM_BASE_URL + AGM_VER │
                       │   • verify checksum        │
                       │   • ./agm --version        │
                       │  On macOS-latest:          │
                       │   • same for darwin-arm64  │
                       │   • arch -x86_64 branch    │
                       │     for darwin-x86_64      │
                       └────────────┬───────────────┘
                                    │
                                 (only on tag v*, gates deploy)
                                    │
                                    ▼
                       ┌────────────────────────────┐
                       │azure-pipelines-deploy.yml  │
                       │  ──────────────────────    │
                       │  Trigger: tag v*, gated    │
                       │          on smoke success  │
                       │                            │
                       │  • Validate tag            │
                       │  • download build artifcts │
                       │  • cargo publish           │
                       │  • GitHubRelease@1         │
                       └────────────────────────────┘

                 ┌───────────────────────────────────────┐
                 │ azure-pipelines-update.yml (Phase B)  │
                 │  ──────────────────────               │
                 │  Trigger: weekly schedule + after     │
                 │           deploy succeeds             │
                 │                                       │
                 │  Matrix: windows-x86_64, macOS-arm64  │
                 │   • install PREVIOUS release          │
                 │   • run agm update                    │
                 │   • verify agm --version == current   │
                 │                                       │
                 │  Skips cleanly on first release       │
                 └───────────────────────────────────────┘
```

**Claves del flujo**:

- `test.yml` corre en *cada* PR y *cada* push a `main`/`release/*`. Rápido, barato, no toca artifacts.
- `build.yml` corre en push a `main` (para smoke los binarios en cada merge, si se desea) **y** en push de tag. Produce artifacts que persisten entre pipelines.
- `smoke.yml` se dispara automáticamente vía `resources.pipelines` cuando `build.yml` completa. El comportamiento es: para un build sobre `main`, smoke corre y reporta. Para un build sobre un tag `v*`, smoke corre y su success es prerequisito para deploy.
- `deploy.yml` se dispara por tag `v*`, pero usa `resources.pipelines` para exigir que el build sobre esa *misma source version* haya completado, y (en Phase B) que smoke haya sido green.
- `update.yml` corre en schedule (una vez por semana, lunes 06:00 UTC) y manualmente por trigger post-deploy.

**Nota sobre `resources.pipelines` y gating**:
Azure Pipelines soporta cross-pipeline triggers pero el gating de "pipeline A requires pipeline B successful on same commit" requiere una combinación de `resources.pipelines.trigger` + `resources.pipelines.source` + check manual del `BuildId`. La implementación concreta debe validarse en el primer Phase B — la alternativa más robusta es **unificar smoke + deploy en un solo pipeline con dos stages** si el gating cross-pipeline resulta frágil. Ver §12.1.

## 5. Inventario de pipelines

> El pseudo-YAML que sigue es **estructural, no canónico**. La sintaxis debe validarse contra la versión actual de Azure Pipelines antes del merge (task YAML linter, `az pipelines validate`, o dry-run en una rama descartable).

### 5.1 `azure-pipelines-test.yml` (rename de `azure-pipelines-build.yml`)

**Propósito**: gate de calidad sobre PRs y pushes no-tag. Rápido (<10 min), determinista, no produce artifacts.

**Trigger**:

```yaml
trigger:
  branches:
    include:
      - main
      - release/*
  paths:
    exclude:
      - '*.md'
      - 'docs/**'
pr:
  branches:
    include:
      - main
```

**Stages**:

1. **Check** (Ubuntu): fmt + clippy.
2. **Test** (paralelo Windows + macOS): `cargo test --workspace`.
3. **Static** (Ubuntu): shellcheck + PSScriptAnalyzer + grep-guard + yamllint.

**Pseudo-YAML estructural**:

```yaml
# azure-pipelines-test.yml  — (renamed from azure-pipelines-build.yml)
# Trigger: PR to main, push to main/release/*

trigger:
  branches:
    include: [main, 'release/*']
  paths:
    exclude: ['*.md', 'docs/**']

pr:
  branches:
    include: [main]

variables:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: '-D warnings'

stages:
  - stage: Check
    displayName: 'Lint & Format'
    jobs:
      - job: Fmt
        pool: { vmImage: 'ubuntu-latest' }
        steps:
          - checkout: self
          - script: |
              curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
                | sh -s -- -y --default-toolchain stable --component rustfmt
              echo "##vso[task.prependpath]$HOME/.cargo/bin"
            displayName: 'Install Rust + rustfmt'
          - script: cargo fmt --all --check
            displayName: 'cargo fmt --check'

      - job: Clippy
        pool: { vmImage: 'ubuntu-latest' }
        steps:
          - checkout: self
          - script: |
              curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
                | sh -s -- -y --default-toolchain stable --component clippy
              echo "##vso[task.prependpath]$HOME/.cargo/bin"
            displayName: 'Install Rust + clippy'
          - script: cargo clippy --workspace --all-targets -- -D warnings
            displayName: 'cargo clippy -D warnings'

  - stage: Test
    displayName: 'Unit tests'
    dependsOn: Check
    jobs:
      - job: TestWindows
        pool: { vmImage: 'windows-latest' }
        steps:
          - checkout: self
          - powershell: |
              Invoke-WebRequest -Uri https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe -OutFile rustup-init.exe
              .\rustup-init.exe -y --default-toolchain stable
              echo "##vso[task.prependpath]$env:USERPROFILE\.cargo\bin"
            displayName: 'Install Rust'
          - script: cargo test --workspace
            displayName: 'cargo test'

      - job: TestMacOS
        pool: { vmImage: 'macOS-latest' }
        steps:
          - checkout: self
          - script: |
              curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
              echo "##vso[task.prependpath]$HOME/.cargo/bin"
            displayName: 'Install Rust'
          - script: cargo test --workspace
            displayName: 'cargo test'

  - stage: Static
    displayName: 'Static checks'
    dependsOn: Check
    jobs:
      - job: Shell
        pool: { vmImage: 'ubuntu-latest' }
        steps:
          - checkout: self
          - script: sudo apt-get update && sudo apt-get install -y shellcheck yamllint
            displayName: 'Install shellcheck + yamllint'
          - script: shellcheck scripts/install.sh
            displayName: 'shellcheck install.sh'
          - script: |
              yamllint -d '{extends: relaxed, rules: {line-length: disable}}' \
                azure-pipelines-test.yml \
                azure-pipelines-build.yml \
                azure-pipelines-smoke.yml \
                azure-pipelines-update.yml \
                azure-pipelines-deploy.yml
            displayName: 'yamllint pipelines'
          - script: |
              # Grep-guard: forbid WiX/.pkg/winresource/gen-icons outside allowlist.
              ALLOWED='cli_install\.md|cd_ci\.md|tests/fixtures/|docs/spec/'
              PATTERNS='wix|cargo-wix|winresource|gen-icons|WixUI|productbuild|pkgbuild|\.icns|installer/macos'
              HITS=$(git grep -n -E "$PATTERNS" -- . | grep -Ev "$ALLOWED" || true)
              if [ -n "$HITS" ]; then
                echo "Forbidden pattern(s) reintroduced:"
                echo "$HITS"
                exit 1
              fi
              echo "Grep-guard passed."
            displayName: 'Grep-guard forbidden patterns'

      - job: PowerShell
        pool: { vmImage: 'windows-latest' }
        steps:
          - checkout: self
          - powershell: |
              # PSScriptAnalyzer is preinstalled on windows-latest.
              Invoke-ScriptAnalyzer -Path scripts/install.ps1 -EnableExit -Severity Warning
            displayName: 'PSScriptAnalyzer install.ps1'
```

**Notas**:

- `paths.exclude` **ya no** excluye `*.yml` — es intencional. Cambios a los pipelines deben disparar test porque test valida los pipelines vía `yamllint` y grep-guard.
- `-Severity Warning` en PSScriptAnalyzer puede cambiarse a `Error` si se quiere tolerar warnings conocidos. Recomendación inicial: `Warning`, relajar si produce ruido.
- Los excludes de paths para el trigger **no aplican** al `pr` trigger — PRs siempre corren todo. Esto es deliberado (un cambio a `README.md` en un PR puede traer cambios colaterales).
- El stage `Static` no depende de `Test`, corre en paralelo a él desde `Check`.

### 5.2 `azure-pipelines-build.yml` (nuevo; reemplaza el antiguo archivo con ese nombre)

**Propósito**: producir pipeline artifacts listos para distribuir. **No publica nada externo**. Es el único pipeline que sabe cómo empaquetar los binarios.

**Trigger**:

```yaml
trigger:
  branches:
    include: [main]
  tags:
    include: ['v*']
  paths:
    exclude: ['*.md', 'docs/**']
```

**Por qué triggerea también en `main`**: así cada merge produce un build "probado" del cual smoke puede tirar automáticamente. Para tags, el build corre igualmente porque el tag apunta a un commit que merecería pasar por main. Si el CI-cost es alto, el trigger puede restringirse a `tags: ['v*']` + push manual.

**Stages**:

1. **Validate** (en tag only): chequeo tag ↔ `Cargo.toml` (igual al actual).
2. **BuildBinaries** (matrix de 3 targets, paralelo).

**Matrix**:

```yaml
strategy:
  matrix:
    windows-x86_64:
      vmImage: 'windows-latest'
      target: 'x86_64-pc-windows-msvc'
      archive_ext: 'zip'
      bin_name: 'agm.exe'
    macos-x86_64:
      vmImage: 'macOS-latest'
      target: 'x86_64-apple-darwin'
      archive_ext: 'tar.gz'
      bin_name: 'agm'
    macos-aarch64:
      vmImage: 'macOS-latest'
      target: 'aarch64-apple-darwin'
      archive_ext: 'tar.gz'
      bin_name: 'agm'
```

**Pseudo-YAML estructural**:

```yaml
# azure-pipelines-build.yml
# Trigger: main + tag v*
# Produces pipeline artifacts: release-<target> × 3

name: 'build-$(Date:yyyyMMdd)$(Rev:.r)'

trigger:
  branches:
    include: [main]
  tags:
    include: ['v*']
  paths:
    exclude: ['*.md', 'docs/**']

variables:
  CARGO_TERM_COLOR: always
  # Only set for tag builds
  TAG_NAME: $[replace(variables['Build.SourceBranch'], 'refs/tags/', '')]

stages:
  - stage: Validate
    displayName: 'Validate tag (only on tag)'
    condition: startsWith(variables['Build.SourceBranch'], 'refs/tags/v')
    jobs:
      - job: ValidateVersion
        pool: { vmImage: 'ubuntu-latest' }
        steps:
          - checkout: self
          - script: |
              curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
              echo "##vso[task.prependpath]$HOME/.cargo/bin"
            displayName: 'Install Rust'
          - script: |
              TAG="$(TAG_NAME)"
              CARGO_VERSION=$(cargo metadata --no-deps --format-version 1 \
                | python3 -c "import sys,json; pkgs=json.load(sys.stdin)['packages']; print([p['version'] for p in pkgs if p['name']=='agm-cli'][0])")
              echo "Tag: $TAG — Cargo.toml: v$CARGO_VERSION"
              if [ "v$CARGO_VERSION" != "$TAG" ]; then
                echo "##vso[task.logissue type=error]Tag $TAG != Cargo.toml v$CARGO_VERSION"
                exit 1
              fi
            displayName: 'Check tag matches Cargo.toml'

  - stage: BuildBinaries
    displayName: 'Build release binaries (3 targets)'
    dependsOn: []           # Run even for main branch (no tag validation).
    jobs:
      - job: Build
        strategy:
          matrix:
            windows-x86_64:
              vmImage: 'windows-latest'
              target: 'x86_64-pc-windows-msvc'
              archive_ext: 'zip'
              bin_name: 'agm.exe'
            macos-x86_64:
              vmImage: 'macOS-latest'
              target: 'x86_64-apple-darwin'
              archive_ext: 'tar.gz'
              bin_name: 'agm'
            macos-aarch64:
              vmImage: 'macOS-latest'
              target: 'aarch64-apple-darwin'
              archive_ext: 'tar.gz'
              bin_name: 'agm'
        pool:
          vmImage: $(vmImage)
        steps:
          - checkout: self

          - script: |
              curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
              source $HOME/.cargo/env
              rustup target add $(target)
            displayName: 'Install Rust + target (unix)'
            condition: ne(variables['Agent.OS'], 'Windows_NT')

          - powershell: |
              Invoke-WebRequest -Uri https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe -OutFile rustup-init.exe
              .\rustup-init.exe -y --default-toolchain stable
              echo "##vso[task.prependpath]$env:USERPROFILE\.cargo\bin"
              rustup target add $(target)
            displayName: 'Install Rust + target (windows)'
            condition: eq(variables['Agent.OS'], 'Windows_NT')

          - script: cargo build --release --target $(target) -p agm-cli
            displayName: 'cargo build (unix)'
            condition: ne(variables['Agent.OS'], 'Windows_NT')

          - script: cargo build --release --target $(target) -p agm-cli
            displayName: 'cargo build (windows)'
            condition: eq(variables['Agent.OS'], 'Windows_NT')

          # Resolve VERSION: use TAG_NAME if tag build; otherwise Cargo.toml version.
          - script: |
              set -e
              if [ -n "$(TAG_NAME)" ] && echo "$(TAG_NAME)" | grep -q '^v'; then
                RESOLVED=$(echo "$(TAG_NAME)" | sed 's/^v//')
              else
                RESOLVED=$(cargo metadata --no-deps --format-version 1 \
                  | python3 -c "import sys,json; pkgs=json.load(sys.stdin)['packages']; print([p['version'] for p in pkgs if p['name']=='agm-cli'][0])")
              fi
              echo "##vso[task.setvariable variable=RESOLVED_VERSION]$RESOLVED"
              echo "Resolved version: $RESOLVED"
            displayName: 'Resolve version'
            condition: ne(variables['Agent.OS'], 'Windows_NT')

          - powershell: |
              if ($env:TAG_NAME -and $env:TAG_NAME.StartsWith('v')) {
                $resolved = $env:TAG_NAME.Substring(1)
              } else {
                $resolved = (cargo metadata --no-deps --format-version 1 |
                  python -c "import sys,json; pkgs=json.load(sys.stdin)['packages']; print([p['version'] for p in pkgs if p['name']=='agm-cli'][0])")
              }
              Write-Host "##vso[task.setvariable variable=RESOLVED_VERSION]$resolved"
              Write-Host "Resolved version: $resolved"
            displayName: 'Resolve version (windows)'
            condition: eq(variables['Agent.OS'], 'Windows_NT')

          - script: |
              set -e
              VERSION="$(RESOLVED_VERSION)"
              STAGE="agm-v${VERSION}-$(target)"
              mkdir -p "dist/v${VERSION}/${STAGE}"
              cp "target/$(target)/release/$(bin_name)" "dist/v${VERSION}/${STAGE}/"
              cp README.md LICENSE "dist/v${VERSION}/${STAGE}/"
              cd "dist/v${VERSION}"
              tar czf "${STAGE}.tar.gz" "${STAGE}"
              shasum -a 256 "${STAGE}.tar.gz" | awk '{print $1"  "$2}' > "${STAGE}.tar.gz.sha256"
              rm -rf "${STAGE}"
            displayName: 'Package tar.gz + sha256'
            condition: ne(variables['archive_ext'], 'zip')

          - powershell: |
              $Version = "$(RESOLVED_VERSION)"
              $Stage = "agm-v$Version-$(target)"
              $TargetDir = "dist/v$Version/$Stage"
              New-Item -ItemType Directory -Force -Path $TargetDir | Out-Null
              Copy-Item "target/$(target)/release/$(bin_name)" $TargetDir
              Copy-Item README.md, LICENSE $TargetDir
              $ZipPath = "dist/v$Version/$Stage.zip"
              Compress-Archive -Path "$TargetDir/*" -DestinationPath $ZipPath -Force
              Remove-Item -Recurse -Force $TargetDir
              $hash = (Get-FileHash $ZipPath -Algorithm SHA256).Hash.ToLower()
              Set-Content -Path "$ZipPath.sha256" -Value "$hash  $Stage.zip"
            displayName: 'Package zip + sha256'
            condition: eq(variables['archive_ext'], 'zip')

          - publish: 'dist'
            artifact: 'release-$(target)'
            displayName: 'Publish artifact release-$(target)'
```

**Notas sobre la estructura del artifact**:

- **Directorio `dist/v<version>/`** contiene **el archivo** y **su `.sha256`**. Esta layout **deliberadamente replica el path que un usuario vería bajo `https://github.com/.../releases/download/v<version>/`**, de modo que cuando smoke lo sirve con `miniserve ./dist`, la URL resultante (`http://localhost:8000/v<version>/<archive>`) tenga el mismo shape que la URL de GitHub.
- El `.sha256` se genera con el formato `<hexdigest>  <filename>` (dos espacios) compatible con `sha256sum -c`.

### 5.3 `azure-pipelines-smoke.yml` (Phase B)

**Propósito**: consumir los pipeline artifacts de `build.yml`, servirlos localmente, y ejercitar los install scripts como lo haría un usuario. **Gating input** para `deploy.yml`.

**Trigger**:

```yaml
trigger: none

pr: none

resources:
  pipelines:
    - pipeline: build
      source: 'agm-cli-build'     # Azure DevOps pipeline name
      trigger:
        branches:
          include: [main]
        tags:
          include: ['v*']
```

**Jobs**: matriz sobre 3 "smoke targets" agrupada por runner:

| Smoke target | Runner | Script | Archive extension |
|---|---|---|---|
| windows-x86_64 | windows-latest | install.ps1 | .zip |
| macos-aarch64 | macOS-latest (apple silicon) | install.sh | .tar.gz |
| macos-x86_64 | macOS-latest (rosetta via `arch -x86_64`) | install.sh | .tar.gz |

**Pseudo-YAML estructural**:

```yaml
# azure-pipelines-smoke.yml
# Triggered by: azure-pipelines-build.yml success
# Purpose: run install.sh/install.ps1 against locally-served build artifacts.

trigger: none
pr: none

resources:
  pipelines:
    - pipeline: build
      source: 'agm-cli-build'
      trigger:
        branches:
          include: [main]
        tags:
          include: ['v*']

variables:
  CARGO_TERM_COLOR: always

stages:
  - stage: Smoke
    displayName: 'Smoke test install scripts'
    jobs:
      - job: SmokeWindows
        displayName: 'Smoke Windows x86_64'
        pool: { vmImage: 'windows-latest' }
        steps:
          - download: build
            artifact: 'release-x86_64-pc-windows-msvc'
            displayName: 'Download windows artifact'

          - powershell: |
              # Install miniserve (has Rust toolchain available, could cache).
              Invoke-WebRequest -Uri https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe -OutFile rustup-init.exe
              .\rustup-init.exe -y --default-toolchain stable
              echo "##vso[task.prependpath]$env:USERPROFILE\.cargo\bin"
            displayName: 'Install Rust'

          - powershell: |
              cargo install miniserve --locked
            displayName: 'Install miniserve'

          - powershell: |
              # Resolve version from artifact layout (expect exactly one v* dir).
              $artifactRoot = "$(Pipeline.Workspace)/build/release-x86_64-pc-windows-msvc"
              $versionDir = Get-ChildItem -Path $artifactRoot -Directory | Where-Object { $_.Name -like 'v*' } | Select-Object -First 1
              $version = $versionDir.Name    # e.g. "v1.1.0-rc.1"
              Write-Host "##vso[task.setvariable variable=SMOKE_VERSION]$version"
              Write-Host "Smoke version: $version"

              # Start miniserve in background serving the artifact root.
              Start-Process -FilePath "miniserve" -ArgumentList "--port","8765","--interfaces","127.0.0.1",$artifactRoot -PassThru
              # Give it time to bind.
              Start-Sleep -Seconds 3
              # Sanity check
              Invoke-WebRequest -Uri "http://127.0.0.1:8765/$version/" -UseBasicParsing | Out-Null
            displayName: 'Start miniserve in background'

          - powershell: |
              $env:AGM_BASE_URL = "http://127.0.0.1:8765/$env:SMOKE_VERSION"
              $env:AGM_VERSION  = "$env:SMOKE_VERSION"
              $env:AGM_INSTALL_DIR = "$env:TEMP\agm-smoke-bin"
              & ./scripts/install.ps1
              & "$env:AGM_INSTALL_DIR\agm.exe" --version
              # Sanity: the version printed by the binary must contain the tag.
              $actual = & "$env:AGM_INSTALL_DIR\agm.exe" --version
              if (-not ($actual -match $env:SMOKE_VERSION.TrimStart('v'))) {
                Write-Error "Installed agm version mismatch: $actual"
                exit 1
              }
            displayName: 'Run install.ps1 and verify'

      - job: SmokeMacOSArm
        displayName: 'Smoke macOS aarch64'
        pool: { vmImage: 'macOS-latest' }
        steps:
          - download: build
            artifact: 'release-aarch64-apple-darwin'
            displayName: 'Download darwin-arm64 artifact'

          - script: |
              curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
              echo "##vso[task.prependpath]$HOME/.cargo/bin"
            displayName: 'Install Rust'

          - script: cargo install miniserve --locked
            displayName: 'Install miniserve'

          - script: |
              set -e
              ARTIFACT_ROOT="$(Pipeline.Workspace)/build/release-aarch64-apple-darwin"
              VERSION=$(ls "$ARTIFACT_ROOT" | grep '^v' | head -n1)
              echo "##vso[task.setvariable variable=SMOKE_VERSION]$VERSION"
              miniserve --port 8765 --interfaces 127.0.0.1 "$ARTIFACT_ROOT" &
              sleep 3
              curl -fsSL "http://127.0.0.1:8765/$VERSION/" > /dev/null
            displayName: 'Start miniserve'

          - script: |
              set -e
              export AGM_BASE_URL="http://127.0.0.1:8765/$(SMOKE_VERSION)"
              export AGM_VERSION="$(SMOKE_VERSION)"
              export AGM_INSTALL_DIR="$HOME/agm-smoke-bin"
              sh ./scripts/install.sh
              "$AGM_INSTALL_DIR/agm" --version
              ACTUAL=$("$AGM_INSTALL_DIR/agm" --version)
              EXPECTED=$(echo "$(SMOKE_VERSION)" | sed 's/^v//')
              if ! echo "$ACTUAL" | grep -q "$EXPECTED"; then
                echo "Installed agm version mismatch: $ACTUAL" >&2
                exit 1
              fi
            displayName: 'Run install.sh and verify'

      - job: SmokeMacOSIntel
        displayName: 'Smoke macOS x86_64 (rosetta)'
        pool: { vmImage: 'macOS-latest' }
        steps:
          - download: build
            artifact: 'release-x86_64-apple-darwin'
            displayName: 'Download darwin-x64 artifact'

          - script: |
              curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
              echo "##vso[task.prependpath]$HOME/.cargo/bin"
            displayName: 'Install Rust'

          - script: cargo install miniserve --locked
            displayName: 'Install miniserve'

          - script: |
              set -e
              ARTIFACT_ROOT="$(Pipeline.Workspace)/build/release-x86_64-apple-darwin"
              VERSION=$(ls "$ARTIFACT_ROOT" | grep '^v' | head -n1)
              echo "##vso[task.setvariable variable=SMOKE_VERSION]$VERSION"
              miniserve --port 8766 --interfaces 127.0.0.1 "$ARTIFACT_ROOT" &
              sleep 3
            displayName: 'Start miniserve'

          - script: |
              set -e
              # Force x86_64 arch for download + install.
              # install.sh uses `uname -m` to pick the triple; on Apple Silicon
              # macOS-latest, `arch -x86_64 sh install.sh` flips uname to x86_64
              # under Rosetta. Rosetta 2 is preinstalled on Azure macOS-latest.
              export AGM_BASE_URL="http://127.0.0.1:8766/$(SMOKE_VERSION)"
              export AGM_VERSION="$(SMOKE_VERSION)"
              export AGM_INSTALL_DIR="$HOME/agm-smoke-bin-x86"
              arch -x86_64 sh ./scripts/install.sh
              # Execute via arch -x86_64 to match target arch.
              arch -x86_64 "$AGM_INSTALL_DIR/agm" --version
            displayName: 'Run install.sh via rosetta and verify'
```

**Riesgos específicos del smoke** (ampliados en §12):

- **`arch -x86_64` en Rosetta**: esto funciona sólo si Rosetta 2 está instalado en el runner. macOS-latest Azure lo tiene en la imagen por default en 2025-2026, pero verificar antes. Plan B: dividir el job en **uno solo** (macos-arm64) y diferir la validación de darwin-x86_64 a la instalación manual en un Mac Intel real, notando el gap en §13.
- **`miniserve` cold install**: ~60-120 s. Aceptable pero cacheable con `Cache@2` si molesta.
- **Port collisions entre macOS aarch64 y macOS x86_64**: si los dos jobs acaban en el mismo runner (no debería, pero Azure no garantiza aislamiento de agent), usar puertos distintos (8765 vs 8766). Ya está reflejado en el pseudo-YAML.

### 5.4 `azure-pipelines-update.yml` (Phase B)

**Propósito**: verificar que `agm update` (subsistema `self_update`) sigue funcionando. Requiere ≥ 2 releases en GitHub. Si solo hay uno, **skipea cleanly**.

**Trigger**:

```yaml
schedules:
  - cron: '0 6 * * 1'     # Monday 06:00 UTC, weekly
    displayName: 'Weekly update test'
    branches:
      include: [main]
    always: true          # Even if no changes

trigger: none
pr: none

resources:
  pipelines:
    - pipeline: deploy
      source: 'agm-cli-deploy'
      trigger:
        tags:
          include: ['v*']
```

**Jobs**: 2 jobs (Windows, macOS aarch64). Cada uno:

1. Obtiene de la GitHub API los dos últimos releases. Si sólo hay uno, log "insufficient releases, skipping" y exit 0.
2. Instala la versión N-1 con `install.sh`/`install.ps1` usando `AGM_VERSION` pinned.
3. Corre `agm update` (no `--check`).
4. Verifica que `agm --version` ahora reporta la versión N.

**Pseudo-YAML estructural**:

```yaml
# azure-pipelines-update.yml
# Purpose: verify `agm update` can hop from release N-1 to release N.

schedules:
  - cron: '0 6 * * 1'
    displayName: 'Weekly update test'
    branches: { include: [main] }
    always: true

trigger: none
pr: none

resources:
  pipelines:
    - pipeline: deploy
      source: 'agm-cli-deploy'
      trigger:
        tags: { include: ['v*'] }

stages:
  - stage: UpdateTest
    jobs:
      - job: UpdateWindows
        pool: { vmImage: 'windows-latest' }
        steps:
          - checkout: self
          - powershell: |
              # Fetch the two latest release tag names.
              $releases = Invoke-RestMethod -Uri 'https://api.github.com/repos/JAAvila-Of/agm-cli/releases?per_page=10'
              $tags = @($releases | Where-Object { -not $_.draft } | ForEach-Object { $_.tag_name })
              if ($tags.Count -lt 2) {
                Write-Host "Only $($tags.Count) release(s) published; skipping update test."
                Write-Host "##vso[task.complete result=SucceededWithIssues;]Only $($tags.Count) release(s)."
                exit 0
              }
              $current  = $tags[0]
              $previous = $tags[1]
              Write-Host "##vso[task.setvariable variable=PREV_VERSION]$previous"
              Write-Host "##vso[task.setvariable variable=CURR_VERSION]$current"
              Write-Host "Previous: $previous  Current: $current"
            displayName: 'Resolve release pair'

          - powershell: |
              if (-not $env:PREV_VERSION) { exit 0 }
              $env:AGM_VERSION = $env:PREV_VERSION
              $env:AGM_INSTALL_DIR = "$env:TEMP\agm-update-bin"
              & ./scripts/install.ps1
              $installed = & "$env:AGM_INSTALL_DIR\agm.exe" --version
              Write-Host "Installed previous: $installed"
              if (-not ($installed -match ($env:PREV_VERSION.TrimStart('v')))) {
                Write-Error "Previous install did not match: $installed"
                exit 1
              }
            displayName: 'Install previous release'

          - powershell: |
              if (-not $env:PREV_VERSION) { exit 0 }
              $env:AGM_INSTALL_DIR = "$env:TEMP\agm-update-bin"
              # `agm update` uses self_update; it replaces $env:AGM_INSTALL_DIR\agm.exe in place.
              & "$env:AGM_INSTALL_DIR\agm.exe" update
              $installed = & "$env:AGM_INSTALL_DIR\agm.exe" --version
              Write-Host "After update: $installed"
              if (-not ($installed -match ($env:CURR_VERSION.TrimStart('v')))) {
                Write-Error "Update did not reach current: $installed, expected $env:CURR_VERSION"
                exit 1
              }
            displayName: 'Run agm update and verify'

      - job: UpdateMacOSArm
        pool: { vmImage: 'macOS-latest' }
        steps:
          - checkout: self
          - script: |
              set -e
              TAGS=$(curl -fsSL 'https://api.github.com/repos/JAAvila-Of/agm-cli/releases?per_page=10' \
                | python3 -c '
              import sys, json
              data = json.load(sys.stdin)
              tags = [r["tag_name"] for r in data if not r["draft"]]
              print(" ".join(tags))')
              set -- $TAGS
              if [ "$#" -lt 2 ]; then
                echo "Only $# release(s) published; skipping update test."
                echo "##vso[task.complete result=SucceededWithIssues;]Only $# release(s)"
                exit 0
              fi
              echo "##vso[task.setvariable variable=CURR_VERSION]$1"
              echo "##vso[task.setvariable variable=PREV_VERSION]$2"
              echo "Previous: $2  Current: $1"
            displayName: 'Resolve release pair'

          - script: |
              set -e
              if [ -z "$(PREV_VERSION)" ]; then exit 0; fi
              export AGM_VERSION="$(PREV_VERSION)"
              export AGM_INSTALL_DIR="$HOME/agm-update-bin"
              sh ./scripts/install.sh
              "$AGM_INSTALL_DIR/agm" --version
            displayName: 'Install previous release'

          - script: |
              set -e
              if [ -z "$(PREV_VERSION)" ]; then exit 0; fi
              export AGM_INSTALL_DIR="$HOME/agm-update-bin"
              "$AGM_INSTALL_DIR/agm" update
              installed=$("$AGM_INSTALL_DIR/agm" --version)
              echo "After update: $installed"
              expected_no_v=$(echo "$(CURR_VERSION)" | sed 's/^v//')
              if ! echo "$installed" | grep -q "$expected_no_v"; then
                echo "Update did not reach current: $installed, expected $(CURR_VERSION)" >&2
                exit 1
              fi
            displayName: 'Run agm update and verify'
```

**Notas críticas**:

- **Assumption de asset name**: `self_update` construye por sí mismo el nombre del archivo a descargar, basándose en el target triple del binario corriendo. El código actual en `crates/agm-cli/src/commands/update.rs` **no** configura `target()`/`identifier()` explícitamente, por lo que usa el comportamiento default de `self_update`. Si el default no incluye el exact string del triple en el filename (ej: busca `agm-x86_64-pc-windows-msvc.zip` sin el `v1.1.0` prefix), el update pipeline fallará sobre los assets construidos por `build.yml`. **Este es el punto exacto donde §11.1 del `cli_install.md` avisó del riesgo**. La primera ejecución en vivo de update.yml **es** la validación final de que el schema de nombres es compatible.
- Si resulta incompatible, la mitigación es añadir `.identifier(format!("agm-v{version}-{target}"))` al builder en `update.rs`, o equivalente. Ese cambio puntual se documenta como task opcional **Task A7** (§10.1).

### 5.5 `azure-pipelines-deploy.yml` (adelgazado)

**Propósito**: publicar a crates.io + crear GitHub Release. **Consume** artifacts de `build.yml`. **En Phase B**, gated por smoke success. **No compila nada**.

**Trigger**:

```yaml
trigger:
  tags:
    include: ['v*']
pr: none

resources:
  pipelines:
    - pipeline: build
      source: 'agm-cli-build'
      trigger:
        tags: { include: ['v*'] }
    # Phase B only:
    - pipeline: smoke
      source: 'agm-cli-smoke'
      trigger:
        tags: { include: ['v*'] }
```

**Pseudo-YAML estructural**:

```yaml
# azure-pipelines-deploy.yml (Phase B — slim version)
# Trigger: tag v*, gated on build + smoke.

trigger:
  tags: { include: ['v*'] }
pr: none

resources:
  pipelines:
    - pipeline: build
      source: 'agm-cli-build'
      trigger:
        tags: { include: ['v*'] }
    - pipeline: smoke            # Phase B only
      source: 'agm-cli-smoke'
      trigger:
        tags: { include: ['v*'] }

variables:
  CARGO_TERM_COLOR: always
  TAG_NAME: $[replace(variables['Build.SourceBranch'], 'refs/tags/', '')]

stages:
  - stage: Publish
    jobs:
      - job: PublishCrate
        pool: { vmImage: 'ubuntu-latest' }
        steps:
          - checkout: self
          - script: |
              curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
              echo "##vso[task.prependpath]$HOME/.cargo/bin"
            displayName: 'Install Rust'
          - script: |
              ENDPOINT_ID=$(az devops service-endpoint list \
                --organization "$(System.CollectionUri)" \
                --project "$(System.TeamProject)" \
                --query "[?name=='Cargo Connection'].id" -o tsv)
              CRATES_TOKEN=$(az devops service-endpoint show \
                --id "$ENDPOINT_ID" \
                --organization "$(System.CollectionUri)" \
                --project "$(System.TeamProject)" \
                --query "authorization.parameters.password" -o tsv)
              echo "##vso[task.setvariable variable=CARGO_REGISTRY_TOKEN;issecret=true]$CRATES_TOKEN"
            displayName: 'Fetch crates.io token'
            env:
              AZURE_DEVOPS_EXT_PAT: $(System.AccessToken)
          - script: |
              cargo publish -p agm-core
              sleep 30
              cargo publish -p agm-cli
            displayName: 'cargo publish core, cli'
            env:
              CARGO_REGISTRY_TOKEN: $(CARGO_REGISTRY_TOKEN)

      - job: GitHubRelease
        pool: { vmImage: 'ubuntu-latest' }
        steps:
          - download: build
            patterns: 'release-*/**'
            displayName: 'Download build artifacts'
          - script: |
              mkdir -p release-assets
              find $(Pipeline.Workspace)/build -type f \
                \( -name 'agm-v*.tar.gz' -o -name 'agm-v*.zip' \
                   -o -name '*.sha256' \) \
                -exec cp {} release-assets/ \;
              ls -la release-assets/
              # Expect 6 files: 3 archives + 3 sha256.
              COUNT=$(ls release-assets | wc -l)
              if [ "$COUNT" -ne 6 ]; then
                echo "Expected 6 files, got $COUNT"
                ls -la release-assets
                exit 1
              fi
            displayName: 'Flatten and verify artifact count'

          - task: GitHubRelease@1
            inputs:
              gitHubConnection: 'GitHub JAAvila-Of'
              repositoryName: 'JAAvila-Of/agm-cli'
              action: 'create'
              target: '$(Build.SourceVersion)'
              tagSource: 'gitTag'
              title: 'agm $(TAG_NAME)'
              releaseNotesSource: 'inline'
              releaseNotesInline: |
                ## Installation

                ### Rust users
                ```sh
                cargo install agm-cli
                ```

                ### One-line installers

                **macOS:**
                ```sh
                curl -fsSL https://raw.githubusercontent.com/JAAvila-Of/agm-cli/main/scripts/install.sh | sh
                ```

                **Windows (PowerShell):**
                ```powershell
                irm https://raw.githubusercontent.com/JAAvila-Of/agm-cli/main/scripts/install.ps1 | iex
                ```

                **Linux:** install via `cargo install agm-cli`.

                ### Manual download

                | Platform             | Archive                                         |
                |----------------------|-------------------------------------------------|
                | Windows x86_64       | `agm-$(TAG_NAME)-x86_64-pc-windows-msvc.zip`    |
                | macOS x86_64         | `agm-$(TAG_NAME)-x86_64-apple-darwin.tar.gz`    |
                | macOS aarch64        | `agm-$(TAG_NAME)-aarch64-apple-darwin.tar.gz`   |

                Each archive ships with a `.sha256` companion.
              assets: 'release-assets/*'
              isDraft: false          # Phase A: true. Phase B: false.
              isPreRelease: false
              changeLogCompareToRelease: 'lastFullRelease'
              changeLogType: 'commitBased'
            displayName: 'Create GitHub Release'
```

**Notas**:

- **Ya no hay `Validate` stage** en deploy: la validación tag ↔ Cargo.toml vive en `build.yml`. Si el tag es malo, `build.yml` falla, sus artifacts no existen, y `deploy.yml` no se dispara (porque `resources.pipelines.trigger` requiere build completed).
- **Ya no hay `BuildBinaries` stage** en deploy: los artifacts llegan vía `download: build`.
- El conteo esperado es **6 archivos** (3 archives + 3 `.sha256`), no 10 como en `cli_install.md`.
- `isDraft`: Phase A lo deja en `true` para rollback fácil si algo rompe el primer release; Phase B lo baja a `false` cuando smoke valida automáticamente.

## 6. Refactor de scripts de instalación

### 6.1 `install.sh` — override `AGM_BASE_URL` + redirección Linux → cargo

**Diff conceptual** (current → new):

**Bloque 1 — Detección OS**:

```sh
# CURRENT
case "$uname_s" in
    Linux)
        case "$uname_m" in
            x86_64 | amd64)  TARGET="x86_64-unknown-linux-gnu"  ;;
            aarch64 | arm64) TARGET="aarch64-unknown-linux-gnu" ;;
            *) echo "error: unsupported Linux arch: $uname_m" >&2; exit 1 ;;
        esac
        ;;
    Darwin)
        ...
    *)
        echo "error: unsupported OS: $uname_s" >&2
        ...
```

```sh
# NEW
case "$uname_s" in
    Linux)
        cat >&2 <<'EOF'
Linux binaries are not published to GitHub Releases.
For Linux, please install via cargo:

    cargo install agm-cli

This installer will be re-enabled for Linux once prebuilt tarballs
ship to GitHub Releases. See https://github.com/JAAvila-Of/agm-cli
EOF
        exit 0
        ;;
    Darwin)
        case "$uname_m" in
            x86_64) TARGET="x86_64-apple-darwin"  ;;
            arm64)  TARGET="aarch64-apple-darwin" ;;
            *) echo "error: unsupported macOS arch: $uname_m" >&2; exit 1 ;;
        esac
        ;;
    *)
        echo "error: unsupported OS: $uname_s" >&2
        echo "       supported: Darwin (macOS)" >&2
        echo "       on Windows, use install.ps1" >&2
        echo "       on Linux, use: cargo install agm-cli" >&2
        exit 1
        ;;
esac
```

**Design note**: `exit 0` on Linux is intentional. A user piping `curl | sh` with no prior knowledge of their platform support should not be scared by a red error; they should get a friendly directive. Exit code 0 signals "nothing wrong happened; here is what to do". If a script consumer really wants to treat it as error (rare), they can check stdout for the banner.

**Bloque 2 — URL construction**:

```sh
# CURRENT
ARCHIVE="agm-${VERSION_TAG}-${TARGET}.tar.gz"
URL="https://github.com/${REPO}/releases/download/${VERSION_TAG}/${ARCHIVE}"
SHA_URL="${URL}.sha256"
```

```sh
# NEW
ARCHIVE="agm-${VERSION_TAG}-${TARGET}.tar.gz"
if [ -n "${AGM_BASE_URL:-}" ]; then
    # Override mode: AGM_BASE_URL points at a directory that holds
    # the archive + .sha256 directly. Used by smoke tests and mirrors.
    URL="${AGM_BASE_URL%/}/${ARCHIVE}"
else
    URL="https://github.com/${REPO}/releases/download/${VERSION_TAG}/${ARCHIVE}"
fi
SHA_URL="${URL}.sha256"
```

**Design note**: `AGM_BASE_URL` is a *base URL*, not a full archive URL. The script still decides the archive filename. This preserves the contract "set version + arch once, download happens automatically" while letting the caller relocate the source.

**Trailing-slash handling**: `${AGM_BASE_URL%/}` strips exactly one trailing slash so that both `http://localhost:8000/v1.1.0` and `http://localhost:8000/v1.1.0/` work identically.

**Usage docstring at top of file**: add

```sh
# Override download source (for mirrors or smoke tests):
#   AGM_BASE_URL=https://mirror.example.com/agm/v1.2.3 AGM_VERSION=v1.2.3 sh install.sh
# The script expects AGM_BASE_URL to contain the archive and its .sha256 directly.
```

**Everything else stays the same**: `AGM_VERSION`, `AGM_INSTALL_DIR`, checksum verification, extraction, PATH hint. No changes.

### 6.2 `install.ps1` — override `AGM_BASE_URL`

**Diff conceptual**:

```powershell
# CURRENT
$Archive = "agm-$VersionTag-$Target.zip"
$Url     = "https://github.com/$Repo/releases/download/$VersionTag/$Archive"
$ShaUrl  = "$Url.sha256"
```

```powershell
# NEW
$Archive = "agm-$VersionTag-$Target.zip"
if ($env:AGM_BASE_URL) {
    $Base = $env:AGM_BASE_URL.TrimEnd('/')
    $Url  = "$Base/$Archive"
} else {
    $Url = "https://github.com/$Repo/releases/download/$VersionTag/$Archive"
}
$ShaUrl = "$Url.sha256"
```

**Usage docstring at top**:

```powershell
# Override download source (for mirrors or smoke tests):
#   $env:AGM_BASE_URL = 'http://localhost:8000/v1.2.3'; $env:AGM_VERSION = 'v1.2.3'; ./install.ps1
# AGM_BASE_URL must contain the archive and its .sha256 directly.
```

Everything else unchanged.

### 6.3 Backward compatibility

- **Default path unchanged**: if `AGM_BASE_URL` is unset, both scripts behave exactly as today.
- **Existing users hitting the one-liner**: no observable change; same URL, same download, same binary.
- **PowerShell `Invoke-WebRequest`** with `http://` (not `https://`) may prompt a warning on some Windows configurations. This is **only** relevant in smoke where `AGM_BASE_URL=http://localhost:...` — users in the wild always get `https://github.com/...`. For smoke, `Invoke-WebRequest` against `127.0.0.1` over HTTP is allowed; no mitigation needed.

## 7. Actualización de documentación

### 7.1 `README.md` — sección Installation

**Table of supported platforms** (replace the 5-row table from `cli_install.md` §9.1):

```markdown
| OS       | Architecture | Archive                                             | Installer |
|----------|--------------|-----------------------------------------------------|-----------|
| Windows  | x86_64       | `agm-vX.Y.Z-x86_64-pc-windows-msvc.zip`             | install.ps1 |
| macOS    | x86_64       | `agm-vX.Y.Z-x86_64-apple-darwin.tar.gz`             | install.sh  |
| macOS    | aarch64      | `agm-vX.Y.Z-aarch64-apple-darwin.tar.gz`            | install.sh  |
| Linux    | any          | *(not published — use `cargo install agm-cli`)*     | cargo       |
```

**Linux block** (explicit, not hidden):

```markdown
### Linux

Prebuilt Linux binaries are not currently published. Install via Cargo:

```sh
cargo install agm-cli
```

This is the recommended path for all Linux users. Linux-targeted tarballs
may be added in a future release.
```

**One-liner block** reworded:

```markdown
### One-line installers

**macOS:**
```sh
curl -fsSL https://raw.githubusercontent.com/JAAvila-Of/agm-cli/main/scripts/install.sh | sh
```

(On Linux, this script will direct you to use `cargo install agm-cli`.)

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/JAAvila-Of/agm-cli/main/scripts/install.ps1 | iex
```
```

**Everything else in the Installation section remains** per `cli_install.md` §9.1: `cargo install`, manual download, from source, uninstall table, checksum verification, security notes.

### 7.2 `docs/releasing.md` — matriz actualizada

**Release Artifacts table** — 3 rows, not 5:

```markdown
| Artifact                                        | Platform       | Notes      |
|-------------------------------------------------|----------------|------------|
| `agm-vX.Y.Z-x86_64-pc-windows-msvc.zip`         | Windows x86_64 | + .sha256  |
| `agm-vX.Y.Z-x86_64-apple-darwin.tar.gz`         | macOS x86_64   | + .sha256  |
| `agm-vX.Y.Z-aarch64-apple-darwin.tar.gz`        | macOS aarch64  | + .sha256  |
```

**CI/CD Pipelines table** — reflect the 5-pipeline split:

```markdown
| Pipeline                        | Trigger                   | Purpose                                  |
|---------------------------------|---------------------------|------------------------------------------|
| azure-pipelines-test.yml        | PR, push main/release/*   | fmt + clippy + tests + static checks     |
| azure-pipelines-build.yml       | push main, tag v*, manual | Matrix build 3 targets; pipeline artifacts |
| azure-pipelines-smoke.yml       | build pipeline success    | install scripts smoke against local HTTP |
| azure-pipelines-update.yml      | schedule + deploy success | `agm update` hop test, N-1 → N           |
| azure-pipelines-deploy.yml      | tag v*                    | cargo publish + GitHub Release           |
```

**ASCII diagram** — replace with the one in §4 above.

**Installation Methods section** — stays deleted per `cli_install.md` §9.2; delegates to README.

### 7.3 `cli_install.md` — optional annotation

Add a one-line note at the top of `cli_install.md`:

```markdown
> **Scope note (2026-04-11):** The target matrix has been narrowed to
> Windows x86_64 + macOS (x86_64, aarch64). Linux targets are deferred;
> Linux users install via `cargo install agm-cli`. See `cd_ci.md` §3.2
> for rationale.
```

This is non-destructive and makes the narrowing discoverable from `cli_install.md` without rewriting its §3.2.

## 8. Chicken-and-egg: manejo del primer release

Dos problemas distintos colisionan en el primer release post-CI-refactor.

### 8.1 Problema 1: primer release no tiene previo

**Síntoma**: `agm update` necesita dos releases para ser ejercitado: uno "previo" para instalar, uno "actual" al que hopear. En el primer release (`v1.1.0`), sólo existe uno.

**Solución**: `update.yml` detecta "releases < 2" y sale con `SucceededWithIssues` + mensaje explícito. Esto está implementado en el pseudo-YAML de §5.4. El primer run (post `v1.1.0`) loggea "skipped, only 1 release"; el segundo run (post `v1.1.1`) ejercita `v1.1.0 → v1.1.1` realmente.

**Consecuencia**: el primer release **no** está cubierto por el update test. Esto es aceptable, y el mitigante es el **manual check one-time** definido en §9: "ejecutar `agm update` sobre una install del release anterior en dos máquinas reales post-v1.1.1". Después de eso, el pipeline automatizado mantiene la cobertura continua.

### 8.2 Problema 2: install.sh apunta a URLs de un release que aún no existe

**Síntoma**: Para ejecutar smoke antes de publicar un release, los install scripts necesitan descargar de *algún lado* los archivos. GitHub Releases no tiene aún el release (¡estamos probándolo!), y montar un draft release es un side-effect visible.

**Solución**: `AGM_BASE_URL` override + `miniserve`. El script **no** mira GitHub Releases cuando `AGM_BASE_URL` está seteado. Descrito en §3.3 y §6.

**Consecuencia**: Phase A rollout se vuelve posible sin smoke pipeline; Phase B añade smoke cuando `AGM_BASE_URL` ya está implementado en los scripts.

### 8.3 Phase A — rollout pre-v1.1.0

**Objetivo**: liberar `v1.1.0` con el pipeline nuevo, **sin** smoke gate, **sin** update test. La validación de `v1.1.0` sigue siendo el checklist manual de `cli_install.md` §12.

**Componentes de Phase A**:

1. Rename `azure-pipelines-build.yml` → `azure-pipelines-test.yml`.
2. Crear nuevo `azure-pipelines-build.yml` con matrix de 3 targets, triggered por main+tag.
3. Adelgazar `azure-pipelines-deploy.yml`: remove BuildBinaries stage (moved to build.yml), add `resources.pipelines: build`, deploy downloads from build artifacts. **No smoke gate**.
4. Refactor `install.sh`: add Linux→cargo redirect, add `AGM_BASE_URL` override.
5. Refactor `install.ps1`: add `AGM_BASE_URL` override.
6. Add static checks stage to `azure-pipelines-test.yml` (shellcheck, PSScriptAnalyzer, grep-guard, yamllint).
7. Add `cd_ci.md` to grep-guard allowlist.
8. Update `README.md` and `docs/releasing.md` to reflect 3-target matrix.
9. Release `v1.1.0` using manual checklist from `cli_install.md` §12.

**End state of Phase A**: 3 pipelines exist (test, build, deploy). Release published. Smoke + update still manual.

### 8.4 Phase B — rollout post-v1.1.0

**Prerequisite**: `v1.1.0` exists as a **non-draft** GitHub Release. At least one previous release (`v1.1.0`) must exist for update.yml to have a "previous" target.

**Objetivo**: automatizar smoke y update; wire smoke as deploy gate.

**Componentes de Phase B**:

1. Crear `azure-pipelines-smoke.yml`. Validate it can consume build artifacts via `resources.pipelines`.
2. Add `resources.pipelines.smoke` to `azure-pipelines-deploy.yml` to gate deploy on smoke success.
3. Crear `azure-pipelines-update.yml` with scheduled + deploy-triggered runs.
4. Crear una patch release `v1.1.1` (puede ser un trivial commit — por ejemplo añadir una línea al CHANGELOG) para:
   - Ejercitar el nuevo smoke gate en vivo.
   - Dar a update.yml un par (v1.1.0, v1.1.1) para probar `agm update` de verdad.
5. Observar el primer run verde de update.yml post-v1.1.1. Si falla por incompatibilidad de asset-name con `self_update`, ejecutar Task A7 (fix del template) y re-release `v1.1.2`.

**End state of Phase B**: 5 pipelines exist. Every tag-release automatically builds, smokes, and deploys. Every week (and after every release), `agm update` is ejercised against real releases.

## 9. Viabilidad por fase del checklist original

Cruce de §12 de `cli_install.md` con los pipelines nuevos:

| Check de `cli_install.md` §12 | Pipeline que lo automatiza | Fase |
|---|---|---|
| 1. `cargo install agm-cli` funciona | No se automatiza en pipeline; cubierto por test.yml indirectamente (cargo test exercises the build). | manual one-time post-release |
| 2. `curl install.sh \| sh` funciona macOS | smoke.yml (SmokeMacOSArm + SmokeMacOSIntel) | Phase B |
| 2. (Linux targets) | **No automatizado** — Linux out of scope | — |
| 3. `irm install.ps1 \| iex` funciona Windows | smoke.yml (SmokeWindows) | Phase B |
| 4. Checksum verification pasa | smoke.yml (install.sh/install.ps1 hacen `Get-FileHash`/`sha256sum -c` internamente) | Phase B |
| 5. `agm update` funciona | update.yml | Phase B (activo desde v1.1.1) |
| 6. CI build pipeline verde | test.yml | Phase A |
| 7. Release pipeline verde end-to-end | build.yml + deploy.yml (Phase A) + smoke.yml gate (Phase B) | Phase A + Phase B |
| 8. Grep de patrones prohibidos = 0 | test.yml stage Static (grep-guard job) | Phase A |
| 9. crates.io muestra versión nueva | No se automatiza (API de crates.io no tiene hook de confirmación); deploy.yml `cargo publish` success es proxy | manual one-time |
| 10. README.md y docs/releasing.md reflejan esquema | test.yml grep-guard + yamllint + review PR | Phase A |
| Visual render del README en GitHub | No se automatiza | manual one-time |

**Cobertura neta**: 8/10 checks automatizados (2 de los 10 ítems son Linux específicos y no aplican). 2 checks quedan manual one-time: el render visual del README, y la confirmación de crates.io post-publish.

## 10. Orden de implementación

Numeración con prefijo `CD-` para distinguir de `cli_install.md`. Phase A es ejecutable sin prerequisitos; Phase B requiere que `v1.1.0` exista como GitHub Release published.

### 10.1 Phase A — pre-v1.1.0

#### Task CD-A1 — Rename test pipeline file

- **Archivos tocados**: rename `H:/PROJECTS/jaavila/agm-cli/azure-pipelines-build.yml` → `H:/PROJECTS/jaavila/agm-cli/azure-pipelines-test.yml`.
- **Comando tipo**: `git mv azure-pipelines-build.yml azure-pipelines-test.yml`
- **Side-effect**: Azure DevOps has a pipeline pointing at the old filename. **Must** update the pipeline definition in Azure UI (or `azure-pipelines.yml` bindings) to point at `azure-pipelines-test.yml`. Note for implementor: do this in the Azure DevOps pipeline settings UI, not via git. Alternatively create a new pipeline bound to the new file and delete/disable the old one.
- **Verificación**: Azure DevOps UI shows the renamed pipeline runs a PR successfully.
- **Rollback**: `git mv azure-pipelines-test.yml azure-pipelines-build.yml` + revert Azure binding.

#### Task CD-A2 — Extend test.yml with static checks

- **Archivo tocado**: `H:/PROJECTS/jaavila/agm-cli/azure-pipelines-test.yml`.
- **Fuente**: stage `Static` from pseudo-YAML in §5.1.
- **Verificación local**: nothing local; push to branch and watch pipeline.
- **Verificación en CI**: stage `Static` green on a PR; shellcheck + PSScriptAnalyzer + grep-guard + yamllint all run and pass.
- **Rollback**: revert the stage addition.

#### Task CD-A3 — Add Linux redirect + AGM_BASE_URL to install.sh

- **Archivo tocado**: `H:/PROJECTS/jaavila/agm-cli/scripts/install.sh`.
- **Fuente**: §6.1 diffs.
- **Verificación local**:
  - `shellcheck scripts/install.sh` locally.
  - Dry-run: `AGM_VERSION=v1.0.0 AGM_INSTALL_DIR=/tmp/agm AGM_BASE_URL=http://example.com/v1.0.0 sh -n scripts/install.sh` (parse-only).
  - Run on a Linux host (WSL o container): expect the Linux→cargo banner on stdout, exit code 0.
- **Rollback**: `git restore scripts/install.sh`.

#### Task CD-A4 — Add AGM_BASE_URL to install.ps1

- **Archivo tocado**: `H:/PROJECTS/jaavila/agm-cli/scripts/install.ps1`.
- **Fuente**: §6.2 diffs.
- **Verificación local**:
  - `Invoke-ScriptAnalyzer scripts/install.ps1` locally.
  - `powershell -NoProfile -Command "Get-Content scripts/install.ps1 | Out-String | Invoke-Expression -ErrorAction SilentlyContinue"` (parse-only — careful, don't actually run the install code path).
  - Better: comment out `Invoke-WebRequest`/`Expand-Archive` calls in a scratch copy and run it.
- **Rollback**: `git restore scripts/install.ps1`.

#### Task CD-A5 — Extract BuildBinaries into new build.yml

- **Archivos tocados**:
  - `H:/PROJECTS/jaavila/agm-cli/azure-pipelines-build.yml` (new file — **not** the renamed one).
  - `H:/PROJECTS/jaavila/agm-cli/azure-pipelines-deploy.yml` (delete BuildBinaries stage).
- **Fuente**: §5.2 pseudo-YAML.
- **Pipeline Azure DevOps binding**: create a new pipeline `agm-cli-build` pointing at `azure-pipelines-build.yml`. Pipeline name matters because `resources.pipelines.source` references it.
- **Matrix change**: **3 targets, not 5**. Drop `linux-x86_64` and `linux-aarch64` entries.
- **Verificación en CI**:
  - Push to a branch; the build pipeline **should not** trigger yet because it's triggered on `main` + tags. Force a manual run via Azure UI.
  - Each of the 3 matrix jobs green.
  - Pipeline artifacts `release-x86_64-pc-windows-msvc`, `release-x86_64-apple-darwin`, `release-aarch64-apple-darwin` appear.
  - Each artifact contains `v<version>/agm-v<version>-<target>.<ext>` and the matching `.sha256`.
- **Rollback**: delete `azure-pipelines-build.yml`, restore `azure-pipelines-deploy.yml`'s BuildBinaries stage from git.

#### Task CD-A6 — Slim deploy.yml, wire resources.pipelines: build

- **Archivo tocado**: `H:/PROJECTS/jaavila/agm-cli/azure-pipelines-deploy.yml`.
- **Fuente**: §5.5 pseudo-YAML (**without** `resources.pipelines.smoke` — that's Phase B).
- **Changes**:
  - Delete `BuildBinaries` stage.
  - Delete `Validate` stage (moved to build.yml).
  - Add `resources.pipelines` block referencing build pipeline.
  - `GitHubRelease` job: change `download: current` → `download: build` + update `find` command to point at `$(Pipeline.Workspace)/build`.
  - Update asset count verification from 10 to 6.
  - Update release notes table from 5 rows to 3 rows (drop Linux rows).
- **Verificación**: no forma de verificar sin tag real. Opción: push a branch con un tag `v1.1.0-rc.0`, observar pipeline, borrar draft release si falla.
- **Rollback**: revert.

#### Task CD-A7 — (Opcional) Audit self_update asset name

- **Archivo tocado**: `H:/PROJECTS/jaavila/agm-cli/crates/agm-cli/src/commands/update.rs`.
- **Propósito**: confirmar (o corregir) que `self_update` construye el nombre de archivo que `build.yml` publica.
- **Acción**:
  - Inspeccionar qué formato default usa `self_update::backends::github::Update` — por defecto busca releases con assets cuyo nombre contiene el target triple + archive extension.
  - Actualmente `update.rs` no fija `.target()` explícitamente. El default puede buscar assets tipo `agm-x86_64-pc-windows-msvc.zip` (sin `v<version>` prefix), **lo cual no coincide con el nombre que build.yml produce** (`agm-v1.1.0-x86_64-pc-windows-msvc.zip`).
  - Si la incompatibilidad se confirma, la fix es una de:
    1. Añadir `.identifier()` al builder con el prefijo `agm-v{version}`.
    2. Configurar `.target()` para matching custom.
    3. Cambiar el nombre del archivo en build.yml para omitir el `v<version>` (menos informativo pero compatible con defaults).
  - **Recomendación**: opción 1. Mantiene el naming del archivo descriptivo y pincha la expectativa del código.
- **Verificación**: unit test en `update.rs` que construye el asset name esperado y lo compara contra una regex.
- **Prioridad**: opcional en Phase A, pero **obligatorio antes del primer run de update.yml en Phase B**. Puede hacerse en Phase A preemptively para evitar sorpresas.
- **Rollback**: revert.

#### Task CD-A8 — Update README and docs/releasing.md

- **Archivos tocados**:
  - `H:/PROJECTS/jaavila/agm-cli/README.md`.
  - `H:/PROJECTS/jaavila/agm-cli/docs/releasing.md`.
- **Fuente**: §7.1 and §7.2 above.
- **Verificación**: manual render check + grep-guard in test.yml (Phase A already green).
- **Rollback**: revert.

#### Task CD-A9 — Release v1.1.0

- **Pasos**:
  1. Bump `crates/agm-cli/Cargo.toml` version to `1.1.0` (if not already). Also bump `agm-core` and commit.
  2. `git tag v1.1.0 && git push --tags`.
  3. Monitor Azure build pipeline and deploy pipeline. Build should produce 3 artifacts; deploy should consume them and publish.
  4. **Manual validation** on real hardware per `cli_install.md` §12:
     - Windows x86_64 host: `irm ...install.ps1 | iex`, `agm --version`.
     - macOS aarch64 host: `curl ...install.sh | sh`, `agm --version`.
     - macOS x86_64 host (if available): same.
  5. If all pass, leave the release published. If any fail, yank crates.io (`cargo yank --vers 1.1.0`), delete the GitHub Release, fix, and re-release as `v1.1.1`.

### 10.2 Phase B — post-v1.1.0

#### Task CD-B1 — Create azure-pipelines-smoke.yml

- **Archivo tocado**: new file `H:/PROJECTS/jaavila/agm-cli/azure-pipelines-smoke.yml`.
- **Fuente**: §5.3 pseudo-YAML.
- **Pipeline Azure DevOps binding**: create new pipeline `agm-cli-smoke`. Configure `resources.pipelines.build.source = 'agm-cli-build'`.
- **Verificación**:
  - Push a trivial no-op commit to main (e.g., whitespace in `CHANGELOG.md`) to trigger build pipeline, which should trigger smoke.
  - SmokeWindows job: downloads artifact, starts miniserve, runs install.ps1, verifies `--version`.
  - SmokeMacOSArm job: same on aarch64.
  - SmokeMacOSIntel job: validates `arch -x86_64` path works on macOS-latest.
- **Risks to check live**:
  - miniserve install time.
  - `resources.pipelines.trigger` cross-pipeline firing.
  - Port binding / job cleanup.
  - Rosetta availability.
- **Rollback**: delete the pipeline binding + the file.

#### Task CD-B2 — Wire smoke as deploy gate

- **Archivo tocado**: `azure-pipelines-deploy.yml`.
- **Change**: add `resources.pipelines.smoke` entry to `resources.pipelines` block, triggered on tags.
- **Verificación**: tag a `v1.1.1` patch release. Expected flow:
  - build.yml runs, publishes artifacts.
  - smoke.yml triggers automatically, runs 3 jobs green.
  - deploy.yml triggers automatically, publishes.
- **Rollback**: remove the smoke reference from deploy.yml.

#### Task CD-B3 — Create azure-pipelines-update.yml

- **Archivo tocado**: new file `H:/PROJECTS/jaavila/agm-cli/azure-pipelines-update.yml`.
- **Fuente**: §5.4 pseudo-YAML.
- **Pipeline Azure DevOps binding**: create new pipeline `agm-cli-update` with a weekly schedule + `resources.pipelines.deploy` trigger.
- **Verificación — initial run**:
  - Manually trigger the pipeline in Azure UI.
  - If only 1 release exists (v1.1.0 only), expect `SucceededWithIssues` on both jobs with "skipping" log.
- **Verificación — post-v1.1.1**:
  - After Task CD-B2 validated with v1.1.1, manually trigger update.yml again.
  - Both jobs should install v1.1.0, run `agm update`, land on v1.1.1.
  - If Task CD-A7 was not done and `self_update` asset matcher is broken, this job fails with a clear "download not found" error; fix with Task CD-A7 and re-release v1.1.2.
- **Rollback**: delete pipeline binding + file.

#### Task CD-B4 — Release v1.1.1 (patch release for Phase B validation)

- **Purpose**: a real tag to drive smoke + update pipeline validation end-to-end.
- **Content**: minimal — a CHANGELOG update noting CI hardening is fine.
- **Steps**:
  1. Bump version to `1.1.1` in `Cargo.toml`.
  2. Tag and push.
  3. Watch the full chain: build → smoke → deploy → update (triggered post-deploy).
- **Acceptance**: green across all 5 pipelines.

## 11. Criterios de aceptación

### 11.1 Phase A — done when

1. `azure-pipelines-test.yml` exists and is bound to a working Azure DevOps pipeline that runs on PRs and pushes to `main`/`release/*`.
2. `test.yml` Static stage runs shellcheck, PSScriptAnalyzer, grep-guard, and yamllint. All four pass on `main` HEAD.
3. Grep-guard fails a PR that reintroduces `wix`/`winresource`/`productbuild`/etc.
4. `azure-pipelines-build.yml` is a **new** file with the 3-target matrix. Manual run succeeds. Artifacts `release-<target>` appear with the expected layout (`v<version>/<archive>` + `<archive>.sha256`).
5. `azure-pipelines-deploy.yml` no longer contains a `BuildBinaries` stage. Deploy's `GitHubRelease` job consumes artifacts via `resources.pipelines: build`. Asset count verification expects exactly 6 files.
6. `scripts/install.sh` supports `AGM_BASE_URL` override and redirects Linux to `cargo install agm-cli` with exit 0.
7. `scripts/install.ps1` supports `AGM_BASE_URL` override.
8. `README.md` and `docs/releasing.md` show the 3-target matrix. No references to Linux prebuilt tarballs remain.
9. `v1.1.0` is published to GitHub Releases and crates.io. Manual install validation on Windows x86_64 and macOS aarch64 passes.

### 11.2 Phase B — done when

10. `azure-pipelines-smoke.yml` exists and is triggered automatically by build pipeline completion. 3 jobs (Windows, macOS aarch64, macOS x86_64 via Rosetta) green on at least one real tag run.
11. `azure-pipelines-update.yml` exists, runs on schedule, and handles the "no previous release" case cleanly with `SucceededWithIssues`.
12. `azure-pipelines-deploy.yml` has a `resources.pipelines.smoke` gate. Releases are blocked if smoke fails.
13. A `v1.1.1` patch release has gone end-to-end through test → build → smoke → deploy → update pipelines, all green.
14. `update.yml` run post-v1.1.1 verifies `agm update` lands from `v1.1.0` to `v1.1.1` on both Windows and macOS aarch64.
15. `cd_ci.md` is in the grep-guard allowlist (no forbidden patterns inside this very document break CI).

## 12. Análisis de riesgos

### 12.1 Cross-pipeline dependency flakiness

**Riesgo**: `resources.pipelines.trigger` in Azure Pipelines has known edge cases around branch/tag matching, especially when the triggering pipeline runs on a tag but the triggered pipeline is checked out on main. Symptoms: smoke is not triggered even though build succeeded, or smoke runs on stale main HEAD rather than the tag commit.

**Probability**: medium. Reports in Azure DevOps community forums.

**Mitigation**:

- Use `resources.pipelines.trigger.tags.include: ['v*']` explicitly on the triggered pipelines.
- Pin `checkout: self` in smoke and deploy jobs to use the commit of the triggering build (via `$(resources.pipeline.build.sourceCommit)`).
- Have a **fallback plan**: collapse smoke into deploy as a stage within the same YAML if cross-pipeline triggering proves flaky after 2-3 tag runs. Smoke-as-stage loses the clean separation but keeps the functional gate.

**Validation**: before tagging `v1.1.1`, validate with a **non-tagged** main push first — push trivial commit to main, observe build fires, observe smoke fires automatically on build success.

### 12.2 miniserve availability

**Riesgo**: `cargo install miniserve --locked` takes 60-120 seconds cold on each runner. On macOS-latest with limited build parallelism, this adds noise to smoke duration.

**Probability**: guaranteed cost, not a failure risk.

**Mitigation**:

- Add `Cache@2` task keyed on `miniserve` + runner OS to cache `~/.cargo/bin/miniserve` across runs.
- Fallback: `python3 -m http.server 8765 &` on macOS (Python preinstalled). For windows-latest, `python -m http.server 8765 --bind 127.0.0.1` also works (Python 3 is preinstalled on Azure windows-latest images as of 2026).

### 12.3 macOS x86_64 under Rosetta

**Riesgo**: Testing the `darwin-x86_64` binary on a `macOS-latest` runner (Apple Silicon) requires Rosetta 2. If Rosetta is not preinstalled or `arch -x86_64` fails, the SmokeMacOSIntel job errors with "Bad CPU type in executable".

**Probability**: low. Rosetta 2 is preinstalled in Azure macOS-latest images. But Azure can update images, and Apple Silicon runners can change defaults.

**Mitigation**:

- Add a pre-step in the job that runs `arch -x86_64 echo ok` and fails-fast if Rosetta is missing, with a clear message.
- Fallback: if Rosetta becomes unavailable, **drop SmokeMacOSIntel** from the smoke matrix and mark `darwin-x86_64` validation as manual-only in `docs/releasing.md`. Document the gap.

### 12.4 First-release gating nothing to gate on

**Riesgo**: Phase A releases `v1.1.0` without smoke. If the scripts have a subtle bug (e.g., a `AGM_BASE_URL` typo in a code path that only matters in the default branch), users hit it first.

**Probability**: medium — this is exactly the risk Phase B closes.

**Mitigation**:

- Phase A includes shellcheck + PSScriptAnalyzer + manual real-hardware validation. These catch most classes of install script bugs.
- The `AGM_BASE_URL` code path added in Phase A is **not exercised by default users** (only set if env var present); its bugs would only manifest for smoke, not for regular users.
- Manual checklist enforcement per `cli_install.md` §12 before Phase A release is non-negotiable.

### 12.5 shellcheck / PSScriptAnalyzer false positives

**Riesgo**: shellcheck warns on patterns like `$HOME/.local/bin` that are fine but trigger SC2016 or similar. PSScriptAnalyzer warns on `Write-Host` (PSAvoidUsingWriteHost). Either blocks merges without real value.

**Probability**: medium. Real PRs will hit this.

**Mitigation**:

- Start with `-Severity Warning` for PSScriptAnalyzer and tolerate warnings as output without failing. Escalate to Error only after tuning.
- For shellcheck, use `# shellcheck disable=SCXXXX` inline on specific lines, not whole-file disable.
- Document in CONTRIBUTING.md how to interpret and silence shellcheck warnings.

### 12.6 Smoke pipeline duration

**Riesgo**: 3 smoke jobs, each ~2-4 minutes (rust install + miniserve install + download + verify), parallel but bounded by slowest. Adds ~8 min to the tag→publish latency.

**Probability**: fact, not risk.

**Mitigation**: acceptable cost. If duration becomes painful, cache `miniserve` via Cache@2, and the duration drops to ~2-3 min.

### 12.7 `agm update` requires ≥ 2 releases

**Riesgo**: discussed in §8.1.

**Mitigation**: Phase B patch release `v1.1.1` provides the second release. Subsequent releases auto-cover each other.

### 12.8 self_update asset name mismatch

**Riesgo**: `self_update` crate's default asset matcher may not find the `build.yml`-produced assets, silently breaking `agm update` at the first real test in update.yml. See Task CD-A7.

**Probability**: unknown without inspecting `self_update` 0.42 default matcher behavior against `agm-v1.1.0-x86_64-pc-windows-msvc.zip`.

**Mitigation**:

- Do Task CD-A7 preemptively in Phase A. Add a unit test that instantiates `self_update::backends::github::Update::configure()` and asserts the constructed asset name regex matches `agm-v\d+\.\d+\.\d+-<triple>\.(zip|tar\.gz)`.
- If mismatch confirmed, use `.identifier()` on the builder to pin the prefix.

### 12.9 Port conflicts on self-hosted shared runners

**Riesgo**: if the Azure pool includes self-hosted agents (not the case for Microsoft-hosted, but possible with custom agents), two jobs running on the same agent might collide on port 8765.

**Probability**: low for Microsoft-hosted `macOS-latest`/`windows-latest` (each job gets a fresh VM). High for self-hosted.

**Mitigation**: use `8765` in SmokeWindows, `8765` in SmokeMacOSArm, `8766` in SmokeMacOSIntel to avoid intra-runner collision in case both macOS jobs ever co-locate. Already reflected in §5.3.

### 12.10 GitHub API rate limiting in update.yml

**Riesgo**: `curl -fsSL 'https://api.github.com/repos/.../releases?per_page=10'` without auth. Unauth limit is 60/hr/IP. Azure runners are shared IPs; weekly cron + per-tag triggers should stay well under, but a flurry of reruns could hit it.

**Probability**: very low.

**Mitigation**: pass `Authorization: Bearer $(System.AccessToken)` or a dedicated GitHub PAT via a secret variable if rate limiting ever appears. Start without; react if problem observed.

## 13. Lo que no se automatiza

Items que permanecen manual checks y no deben intentar automatizarse en este plan:

1. **Visual render del README en GitHub UI**: nadie ve cómo GitHub renderiza el README excepto un humano mirando. Permanece manual.
2. **Confirmación de crates.io post-publish**: `cargo publish` success es proxy. La UI de crates.io al día de la publicación (versión visible, badges green) requiere inspección humana una vez por release.
3. **`cargo install agm-cli` real en Linux**: el único "canal Linux" que ofrecemos. Ejercitado trivialmente al dar `cargo install` verde en cualquier contributor's box; no se automatiza en pipeline (porque automatizarlo equivaldría a añadir un target Linux al build, lo cual es precisamente lo que descartamos).
4. **First `agm update` across the pre- and post-refactor naming scheme**: si un usuario instaló `v1.0.0` (con MSI/pkg) y corre `agm update` pointed at `v1.1.0+` (con tarball/zip), el subsistema `self_update` puede fallar. Esto es una transición one-way que ocurre exactamente una vez en la historia del proyecto y tiene mitigación documentada en release notes de `v1.1.0` (§11.1 de `cli_install.md`). No tiene sentido automatizarla.
5. **Code signing warnings**: si aparecen en Windows SmartScreen / macOS Gatekeeper en el futuro, la resolución no es automatizable — requiere comprar certificados, integrar firma en build.yml, etc. Fuera de alcance.

## 14. Tracks de seguimiento (out-of-scope)

Listado para memoria institucional; ninguno es parte de este plan.

1. **Linux prebuilt tarballs**: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-unknown-linux-musl`. Mecánicamente: una entrada por target en matrix de build.yml + matrix de smoke.yml + restoring the Linux branch in `install.sh`.
2. **Homebrew tap** `JAAvila-Of/homebrew-agm`: formula update job after deploy.
3. **winget manifest**: PR al repo `microsoft/winget-pkgs` con `wingetcreate` al finalizar deploy.
4. **Scoop bucket** `JAAvila-Of/scoop-agm`.
5. **Chocolatey package**: nuspec + push a community repo.
6. **Code signing**:
   - Authenticode cert for Windows (~USD 250-900/year).
   - Apple Developer ID + `notarytool` for macOS (~USD 99/year + notarization).
7. **Shell completions + man pages**: `clap_complete` + `clap_mangen` at build time, include in tarballs, copy to `$INSTALL_DIR/../share/...` in install scripts.
8. **yamllint strict ruleset**: right now §5.1 uses `-d '{extends: relaxed, rules: {line-length: disable}}'`. Tighten progressively.
9. **cargo audit / cargo deny**: dedicated `azure-pipelines-audit.yml` running on a schedule with advisory-db refresh.
10. **Replace Azure Pipelines with GitHub Actions**: future migration, out of scope.
11. **Unified release dashboard** consolidating test + build + smoke + deploy + update status across pipelines.

---

## Apéndice A — Referencias cruzadas a `cli_install.md`

| Sección de este plan | Referencia `cli_install.md` |
|---|---|
| §3.2 (3 targets, no 5) | §3.2 ("aarch64 Linux recomendada pero puede diferirse") |
| §3.3 (`AGM_BASE_URL`) | §8.1 (`AGM_VERSION`, `AGM_INSTALL_DIR` precedent) |
| §3.5 (smoke gate) | §11.7 (`isDraft: true` during rc) |
| §5.5 (thin deploy) | §7.3, §7.4 (PublishCrate + GitHubRelease preserved) |
| §6.1 (install.sh redirect) | §8.1 (original spec) |
| §6.2 (install.ps1 override) | §8.2 (original spec) |
| §8 (chicken-and-egg) | §11.1 (self_update asset naming), §11.7 (first pipeline failure) |
| §9 (checklist automation) | §12 (manual checklist) |
| §12.8 (self_update mismatch) | §11.1 (exact same risk) |
| §13 (manual one-time checks) | §12.3 (acceptance criteria) |

## Apéndice B — Pipeline Azure DevOps bindings (names)

| YAML file | Azure DevOps pipeline name | Resource ref |
|---|---|---|
| `azure-pipelines-test.yml` | `agm-cli-test` | — |
| `azure-pipelines-build.yml` | `agm-cli-build` | referenced as `build` in smoke.yml and deploy.yml |
| `azure-pipelines-smoke.yml` | `agm-cli-smoke` | referenced as `smoke` in deploy.yml (Phase B) |
| `azure-pipelines-deploy.yml` | `agm-cli-deploy` | referenced as `deploy` in update.yml |
| `azure-pipelines-update.yml` | `agm-cli-update` | — |

## Apéndice C — Variables de entorno usadas en los scripts

| Variable | Consumida por | Propósito | Default |
|---|---|---|---|
| `AGM_VERSION` | install.sh, install.ps1 | Pin a specific release tag | latest from GitHub API |
| `AGM_INSTALL_DIR` | install.sh, install.ps1 | Override install destination | `$HOME/.local/bin` (unix), `$env:LOCALAPPDATA\Programs\agm\bin` (windows) |
| `AGM_BASE_URL` | install.sh, install.ps1 (new) | Override download base URL | `https://github.com/JAAvila-Of/agm-cli/releases/download/${AGM_VERSION}` |

## Apéndice D — Secuencia de decisiones pendientes (para revisión del usuario)

Items en este plan que requieren confirmación antes de ejecutar:

1. **Phase A vs. Phase B split**: ¿aceptable liberar `v1.1.0` con smoke+update aún manual?
2. **Rosetta on macOS-latest**: aceptable asumir que está preinstalado, o requerir un pre-check?
3. **`miniserve` vs. `python3 -m http.server`**: `miniserve` añade ~60s build. ¿Aceptable?
4. **Task CD-A7 preemptive**: hacer la auditoría + fix de `update.rs` en Phase A aunque aún no se ejercite? Recomendación fuerte: **sí**.
5. **`isDraft: false` timing**: en Phase A el release se publica como draft y se promueve manualmente, o directo a public después de manual checklist?
6. **Update pipeline cron timing**: lunes 06:00 UTC. ¿Horario correcto para el equipo?
7. **grep-guard severity**: fail-fast (error) o warn-only durante un primer periodo? Recomendación: fail-fast desde día uno.
8. **Remove `windows-latest` y `macOS-latest` labels en favor de versiones pinneadas** (e.g. `windows-2022`, `macos-14`)? Out-of-scope, pero el usuario puede preferir pinnear para reproducibilidad.
