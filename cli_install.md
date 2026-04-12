# Plan: simplificación del stack de distribución de `agm-cli`

## 1. Resumen ejecutivo

El repositorio `agm-cli` acumula una superficie de instaladores considerable (WiX/MSI para Windows, productbuild/.pkg universal para macOS, generador de íconos en Python con Pillow, `build.rs` + `winresource` para incrustar VERSIONINFO en `agm.exe`, plantillas HTML de wizard, recursos gráficos `.ico`/`.icns`/`.bmp`/`.png`, dos stages pesados en el pipeline de release). Este modelo duplica el trabajo de lo que ofrece `cargo install`, introduce una cadena de herramientas frágil (cargo-wix, pkgbuild, productbuild, lipo, Rez/SetFile, Pillow), y a futuro arrastrará costes de firma de código (USD 250-900/año) para evitar avisos de SmartScreen/Gatekeeper.

El objetivo es alinear la distribución con el patrón estándar que usan los CLIs de Rust más adoptados (ripgrep, fd, bat, starship, zoxide, helix): **(a)** `cargo install agm-cli` como canal principal para usuarios Rust, **(b)** archivos pre-compilados (`.tar.gz` / `.zip`) por tripleta de target publicados en GitHub Releases con checksums `.sha256`, y **(c)** scripts `install.sh` / `install.ps1` de una línea que detectan plataforma, descargan el archivo correcto, verifican el checksum y colocan el binario en el PATH del usuario. Todo el andamiaje WiX/.pkg/build.rs/winresource/Pillow desaparece.

El beneficio neto es: menos código y metadata a mantener, pipeline de release mucho más simple y paralelizable (build-matrix por tripleta), eliminación total de la dependencia futura de certificados de firma, onboarding del contribuidor más rápido (sin toolchain de instaladores), y paridad con la forma en que los desarrolladores del ecosistema Rust *esperan* instalar una herramienta CLI.

## 2. Alcance y no-objetivos

### 2.1 En alcance

- Eliminación completa de la infraestructura WiX/MSI.
- Eliminación completa de la infraestructura `.pkg`/productbuild/pkgbuild/Rez/SetFile de macOS.
- Eliminación de `build.rs` y la dependencia `winresource` en `crates/agm-cli`.
- Eliminación del generador Python `scripts/gen-icons.py` y de todos los assets gráficos que genera.
- Reestructuración de `azure-pipelines-deploy.yml` para producir archivos `.tar.gz`/`.zip` pre-compilados por tripleta + `.sha256` por archivo.
- Adición de targets Linux (`x86_64-unknown-linux-gnu` y opcionalmente `aarch64-unknown-linux-gnu`) que hoy no se construyen.
- Actualización de `scripts/install.sh` (actualmente solo macOS) y `scripts/install.ps1` (ya existe, casi correcto) para soportar detección multi-plataforma y verificación de checksum.
- Actualización de `scripts/build-release.sh` para reflejar el nuevo esquema local (o su eliminación si ya no aporta valor sobre el pipeline de CI).
- Actualización de la sección "Installation" de `README.md` y del documento `docs/releasing.md`.
- Decisión sobre `logo.png` (mantener como asset de branding para README/docs; ya no es input de build).

### 2.2 No-objetivos (seguimiento futuro, no planificados aquí)

- Homebrew tap (`homebrew-agm`).
- Scoop bucket y manifest para winget.
- Code signing (Authenticode en Windows, Developer ID + notarización en macOS).
- Paquetes nativos de Linux (`.deb`, `.rpm`, AUR).
- Builds musl estáticos (`x86_64-unknown-linux-musl`).
- Shell completions y man pages (si `agm` no los emite hoy vía clap, es trabajo aparte).
- Cualquier reescritura del comando `agm update` / feature `self-update` (ese subsistema sigue apuntando a GitHub Releases, y los nuevos nombres de archivo deben ser compatibles con lo que `self_update` espera; ver sección 11).

## 3. Rationale de diseño

### 3.1 Comparativa con CLIs Rust populares

| Proyecto | Windows | macOS | Linux | Método principal | Instalador nativo |
|----------|---------|-------|-------|------------------|-------------------|
| ripgrep  | `.zip` (msvc) | `.tar.gz` (x64+arm64 separados) | `.tar.gz` (gnu+musl) + `.deb` | GitHub Releases + cargo | No `.msi`, no `.pkg` |
| fd       | `.zip` (msvc) | `.tar.gz` (x64+arm64 separados) | `.tar.gz` (gnu+musl) + `.deb` | GitHub Releases + cargo | No |
| bat      | `.zip` | `.tar.gz` | `.tar.gz` + `.deb` | GitHub Releases + cargo | No |
| starship | `.zip` + instalador shell | `.tar.gz` | `.tar.gz` | `install.sh` / scoop / brew | No |
| zoxide   | `.zip` | `.tar.gz` | `.tar.gz` | `install.sh` / brew / scoop | No |
| helix    | `.zip` | `.tar.gz` | `.tar.gz` + AppImage | GitHub Releases | No |

Ninguno de los CLIs de Rust de uso masivo en 2026 distribuye MSI o .pkg. El motivo es práctico: los usuarios de Rust ya tienen `cargo install`; los que no son usuarios de Rust usan gestores del ecosistema (brew, scoop) o el `install.sh` de una línea. El MSI/.pkg ofrece Add/Remove Programs y wizard UI, pero a cambio exige firmado para evitar warnings, y la experiencia "de una línea" es lo que hoy marca el estándar UX.

### 3.2 Tripletas objetivo

Mínimo recomendado para el primer release post-refactor:

| Tripleta | Archivo | Runner CI | Justificación |
|----------|---------|-----------|---------------|
| `x86_64-pc-windows-msvc` | `.zip` | `windows-latest` | Windows x64 es ~95% del Windows instalado. |
| `x86_64-unknown-linux-gnu` | `.tar.gz` | `ubuntu-latest` | Linux x64 mainstream. **No se construye hoy.** |
| `aarch64-unknown-linux-gnu` | `.tar.gz` | `ubuntu-latest` (con `cross` o docker) u ofrecer como opcional inicialmente | Servidores ARM (AWS Graviton, Ampere), Raspberry Pi, Asahi. |
| `x86_64-apple-darwin` | `.tar.gz` | `macOS-latest` | Macs Intel. Hoy se compila, hoy se lipo-fusiona en universal. |
| `aarch64-apple-darwin` | `.tar.gz` | `macOS-latest` | Macs Apple Silicon. Hoy se compila, hoy se lipo-fusiona. |

**Decisión: abandonar el universal binary de macOS.** Shipping de dos tarballs separados (`x86_64-apple-darwin` y `aarch64-apple-darwin`) es lo que hacen ripgrep, fd y bat. Ventajas: (a) el instalador automático detecta `uname -m` y baja solo lo que necesita (≈50% menos descarga), (b) se elimina el paso `lipo`, (c) simplifica el mapeo de tripleta→archivo, (d) elimina el concepto "universal" del nombre del asset.

**Decisión: gnu en vez de musl para Linux.** `gnu` es más simple (no requiere `cross` + docker), compila en `ubuntu-latest` directamente, y cubre el 99% de las distros modernas. `musl` se deja como track de seguimiento si aparece demanda de portabilidad estática (Alpine, distroless).

**Decisión sobre `aarch64-unknown-linux-gnu`:** recomendada pero puede diferirse al segundo release. Si se incluye desde el día uno, la ruta más barata es instalar `cross` en el runner Ubuntu y hacer `cross build --target aarch64-unknown-linux-gnu`. No requiere runner ARM nativo.

### 3.3 Formato de archivo

- **Unix** (`*-linux-gnu`, `*-apple-darwin`): `.tar.gz`. Es lo que `tar` lee sin flags adicionales y es universal.
- **Windows**: `.zip`. PowerShell `Expand-Archive` lo lee nativamente sin 7-zip.

Contenido de cada archivo (homogéneo en todas las plataformas):

```
agm-v1.0.0-<target>/
  agm(.exe)
  README.md
  LICENSE
```

Se pliega dentro de un directorio con el nombre `agm-v<version>-<target>` (como hace ripgrep) para que al descomprimir no se ensucie el `cwd`.

### 3.4 Estrategia de checksums

Para cada archivo publicado se genera un `.sha256` hermano:

```
agm-v1.0.0-x86_64-pc-windows-msvc.zip
agm-v1.0.0-x86_64-pc-windows-msvc.zip.sha256
agm-v1.0.0-x86_64-unknown-linux-gnu.tar.gz
agm-v1.0.0-x86_64-unknown-linux-gnu.tar.gz.sha256
...
```

Formato del `.sha256` (una línea, compatible con `sha256sum -c`):

```
<hexdigest>  agm-v1.0.0-x86_64-unknown-linux-gnu.tar.gz
```

**Decisión: un `.sha256` por archivo, no un SHASUMS256.txt agregado.** Razón: el instalador `install.sh` descarga *un* archivo y necesita *un* checksum. Un SHASUMS256.txt agregado obligaría al script a parsearlo; un `.sha256` por archivo se descarga y verifica con dos líneas de shell. ripgrep usa esta estrategia.

### 3.5 Ubicación de los scripts de instalación

Se mantienen en `scripts/install.sh` y `scripts/install.ps1`. Razones:
- Ya están ahí. Los usuarios que hayan visto un README anterior esperan esa ruta.
- La URL `raw.githubusercontent.com/JAAvila-Of/agm-cli/main/scripts/install.sh` es estable y cacheable.
- No hay razón para moverlos a `.github/` ni a la raíz: `scripts/` es el convenio de starship, zoxide y mcfly.

**Riesgo conocido** (ver sección 11): `raw.githubusercontent.com/.../main/scripts/install.sh` sirve *siempre* la versión en `main`. Si el script asume un esquema de asset que existe solo desde v1.1.0 y un usuario antiguo ya ejecutó el curl una vez y lo tiene cacheado en su historial, puede ejecutar un script que espera assets que no existían en un tag anterior. Mitigación: el script siempre resuelve "latest release" dinámicamente, no asume un esquema hard-codeado de versión.

## 4. Inventario: todo lo que se elimina

Enumeración exhaustiva obtenida vía `Glob`/`Read`/`Grep` sobre `H:/PROJECTS/jaavila/agm-cli`. Cada ruta es absoluta con slashes forward.

### 4.1 Directorio WiX completo

```
H:/PROJECTS/jaavila/agm-cli/crates/agm-cli/wix/
H:/PROJECTS/jaavila/agm-cli/crates/agm-cli/wix/main.wxs
H:/PROJECTS/jaavila/agm-cli/crates/agm-cli/wix/License.rtf
H:/PROJECTS/jaavila/agm-cli/crates/agm-cli/wix/agm.ico
H:/PROJECTS/jaavila/agm-cli/crates/agm-cli/wix/WixUIBanner.bmp
H:/PROJECTS/jaavila/agm-cli/crates/agm-cli/wix/WixUIDialog.bmp
```

| Archivo | Qué hace hoy | Por qué es seguro eliminar |
|---------|--------------|----------------------------|
| `main.wxs` | Manifest WiX Toolset: define `<Product>`, `WixUI_InstallDir`, `MajorUpgrade`, metadata ARP (ARPPRODUCTICON, ARPHELPLINK, ARPNOMODIFY/ARPNOREPAIR), componente `binary0` que instala `agm.exe` y componente `Path` que añade el install dir al PATH del sistema. | Solo lo consume `cargo wix` en el stage `BuildWindows`. Al eliminarse ese stage, no queda consumidor. Nada del código Rust referencia este archivo. |
| `License.rtf` | Texto RTF de Apache-2.0 mostrado en el wizard WiX. | Referenciado únicamente por `main.wxs` (`WixUILicenseRtf`). Al desaparecer el wizard, es huérfano. El `LICENSE` raíz sigue siendo la fuente canónica. |
| `agm.ico` | Multi-size `.ico` (16/24/32/48/64/128/256) generado por `gen-icons.py`. Referenciado por `main.wxs` (ARPPRODUCTICON) **y** por `crates/agm-cli/build.rs:14` (`res.set_icon("wix/agm.ico")`). | Al eliminar ambos consumidores desaparece. |
| `WixUIBanner.bmp` | Banner superior 493×58 del wizard, referenciado por `main.wxs` (`WixUIBannerBmp`). | Huérfano al desaparecer el wizard. |
| `WixUIDialog.bmp` | Diálogo welcome/complete 493×312, referenciado por `main.wxs` (`WixUIDialogBmp`). | Huérfano al desaparecer el wizard. |
| Directorio `wix/` | Contenedor del stack WiX. | Vacío tras eliminar sus contenidos. |

### 4.2 Directorio macOS installer completo

```
H:/PROJECTS/jaavila/agm-cli/crates/agm-cli/installer/
H:/PROJECTS/jaavila/agm-cli/crates/agm-cli/installer/macos/
H:/PROJECTS/jaavila/agm-cli/crates/agm-cli/installer/macos/agm.icns
H:/PROJECTS/jaavila/agm-cli/crates/agm-cli/installer/macos/distribution.xml.tmpl
H:/PROJECTS/jaavila/agm-cli/crates/agm-cli/installer/macos/uninstall.sh
H:/PROJECTS/jaavila/agm-cli/crates/agm-cli/installer/macos/Resources/
H:/PROJECTS/jaavila/agm-cli/crates/agm-cli/installer/macos/Resources/background.png
H:/PROJECTS/jaavila/agm-cli/crates/agm-cli/installer/macos/Resources/welcome.html
H:/PROJECTS/jaavila/agm-cli/crates/agm-cli/installer/macos/Resources/readme.html
H:/PROJECTS/jaavila/agm-cli/crates/agm-cli/installer/macos/Resources/license.html
H:/PROJECTS/jaavila/agm-cli/crates/agm-cli/installer/macos/Resources/conclusion.html
```

| Archivo | Qué hace hoy | Por qué es seguro eliminar |
|---------|--------------|----------------------------|
| `agm.icns` | Icono Apple multi-size (ic07..ic14, 16..1024 px) generado por `gen-icons.py`. Se copia a `/usr/local/share/agm-cli/agm.icns` como parte del payload del pkg, y se vuelve a copiar a `/tmp/agm-pkg-icon.icns` para que Rez/SetFile lo incruste como icono Finder del propio `.pkg`. | Solo lo usa el stage `BuildMacOS` (`azure-pipelines-deploy.yml:154-157` y `:193-197`). Al desaparecer el stage, no queda consumidor. |
| `distribution.xml.tmpl` | Template `productbuild` con `__VERSION__` substituido por `sed`. Define `<welcome/readme/license/conclusion>` file refs, background para darkAqua, `hostArchitectures="x86_64,arm64"`, `volume-check min 10.13`. | Solo lo consume el pipeline en `:179-181`. |
| `uninstall.sh` | Script bash que `sudo` ejecuta: borra `/usr/local/bin/agm`, limpia `/usr/local/share/agm-cli/`, hace `pkgutil --forget com.jaavila.agm`. | Se embarca dentro del `.pkg` (`:156-158`). Al no haber `.pkg`, el instalador del tarball no instala en `/usr/local/share/agm-cli/`, así que no hay nada que desinstalar más allá de `rm $HOME/.local/bin/agm`. |
| `Resources/background.png` | 620×418, fondo del wizard Installer.app. | Huérfano al eliminar `distribution.xml.tmpl`. |
| `Resources/welcome.html` | Página welcome del wizard. | Huérfano. |
| `Resources/readme.html` | Página readme del wizard. | Huérfano. |
| `Resources/license.html` | Página license del wizard. | Huérfano. |
| `Resources/conclusion.html` | Página final del wizard — también contiene instrucciones de uninstall hard-codeadas que ya no aplicarán. | Huérfano. |
| Directorios `installer/`, `installer/macos/`, `installer/macos/Resources/` | Contenedores. | Vacíos tras el cleanup. |

### 4.3 Build script Rust + dependencia

```
H:/PROJECTS/jaavila/agm-cli/crates/agm-cli/build.rs
```

| Archivo | Qué hace hoy | Por qué es seguro eliminar |
|---------|--------------|----------------------------|
| `build.rs` | En Windows, instancia `winresource::WindowsResource`, le asigna el icono `wix/agm.ico`, fija strings VERSIONINFO (FileDescription, ProductName, CompanyName, LegalCopyright, OriginalFilename, InternalName), y los compila. Además declara `cargo:rerun-if-changed=wix/agm.ico` y `Cargo.toml`. Fuera de Windows es no-op. | Su única función es estética en Windows (que el ejecutable tenga icono propio y metadata en "Properties → Details"). Al distribuir el `.exe` dentro de un `.zip` de un release de GitHub, esta estética no afecta la instalación ni el uso. Los CLIs de Rust populares (ripgrep, fd) distribuyen `.exe` sin VERSIONINFO/icono y no ha sido una queja del usuario. |

Efecto cascada en `crates/agm-cli/Cargo.toml` (ver sección 5).

### 4.4 Generador Python y assets de origen

```
H:/PROJECTS/jaavila/agm-cli/scripts/gen-icons.py
```

| Archivo | Qué hace hoy | Por qué es seguro eliminar |
|---------|--------------|----------------------------|
| `gen-icons.py` | Pillow-based generator. Lee `logo.png` y produce `agm.ico`, `WixUIBanner.bmp`, `WixUIDialog.bmp`, `agm.icns`, `Resources/background.png`. | Al eliminar todos sus outputs, el generador no tiene razón de existir. Elimina dependencia implícita en Python 3 + Pillow para contribuidores. |

### 4.5 `logo.png` de la raíz

```
H:/PROJECTS/jaavila/agm-cli/logo.png
```

**Decisión: mantener** pero documentar que ya no es input de ningún build. Razones:
- El README puede referenciar `![logo](logo.png)` en el futuro para branding.
- Cuesta ~0 en tamaño de repo.
- Eliminarlo cierra la puerta a futura tap Homebrew con icono, docs.rs con logo, sitio web del proyecto, etc.

Si el usuario prefiere eliminarlo, es seguro hacerlo: `Grep` confirma que tras eliminar `gen-icons.py` ningún otro archivo lo referencia. **Nótese que `logo.png` aparece en `git status` como untracked en el momento de este análisis**, así que ni siquiera está comprometido al repo todavía — la decisión de mantener/eliminar es equivalente a "añadirlo al commit o no añadirlo".

### 4.6 Resumen de stages de pipeline eliminados

Del archivo `azure-pipelines-deploy.yml` (no el archivo en sí, sí su contenido):

- Bloque de comentario del Test stage deshabilitado (líneas 49-79): puede eliminarse limpiamente ya que no aporta información viva.
- Stage `BuildWindows` (líneas 81-112) completo: elimina `cargo install cargo-wix`, `cargo wix`, publicación de `windows-msi`.
- Stage `BuildMacOS` (líneas 114-210) completo: elimina `lipo`, `pkgbuild`, `productbuild`, bloque Rez/SetFile, referencias a `distribution.xml.tmpl` y a `installer/macos/`, publicación de `macos-pkg`.

Estos stages se reemplazan (no se borran sin más), ver sección 7.

## 5. Inventario: todo lo que se modifica

### 5.1 `H:/PROJECTS/jaavila/agm-cli/crates/agm-cli/Cargo.toml`

**Propósito actual**: manifest del binario CLI. Declara dependencias, features, y una sección Windows-only de build-dependencies.

**Cambio puntual**: eliminar exactamente las líneas 40-43:

```toml
# Embed Win32 VERSIONINFO + icon into agm.exe on Windows hosts.
# winresource is the maintained fork of the stale `winres` crate.
[target.'cfg(windows)'.build-dependencies]
winresource = "0.1"
```

**Verificación post-cambio**: `cargo metadata --no-deps | grep winresource` no debe devolver nada. `Cargo.lock` se regenerará automáticamente en el siguiente `cargo build`, y `winresource` (actualmente en `Cargo.lock:36` y `:2674`) debe desaparecer.

### 5.2 `H:/PROJECTS/jaavila/agm-cli/crates/agm-cli/build.rs`

**Propósito actual**: incrustar VERSIONINFO + icono en `agm.exe` en hosts Windows.

**Cambio**: eliminar el archivo completo. No reemplazar con un stub: Cargo solo ejecuta `build.rs` si existe, así que borrarlo equivale a "no hacer nada en build".

### 5.3 `H:/PROJECTS/jaavila/agm-cli/Cargo.lock`

**Propósito actual**: lockfile workspace.

**Cambio**: se regenera automáticamente tras eliminar `winresource`. **No editar a mano.** Debe commitearse tras el primer `cargo build` local. Se espera que desaparezcan las entradas `winresource`, `embed-resource`, `toml` transitivas que solo winresource traía, y cualesquiera otras.

### 5.4 `H:/PROJECTS/jaavila/agm-cli/azure-pipelines-deploy.yml`

**Propósito actual**: pipeline disparado en tag `v*`. Stages: Validate → BuildWindows(MSI+zip) → BuildMacOS(pkg+tar.gz) → PublishCrate → GitHubRelease. El stage Test está comentado.

**Cambio**: reestructuración completa de stages 3-6. La versión nueva se detalla en la sección 7. Resumen de la transformación:

- **Validate** (actual): sin cambios.
- **Test deshabilitado** (bloque comentado): eliminar completamente ese bloque de comentarios para higiene; el test real vive en `azure-pipelines-build.yml` vinculado a PRs/push, ya es gating.
- **BuildWindows** (actual): reemplazar por job dentro de un stage `BuildBinaries` con estrategia matrix que cubre las 5 tripletas.
- **BuildMacOS** (actual): eliminar por completo (absorbido por el matrix).
- **PublishCrate**: sin cambios funcionales; la única modificación es que ahora `dependsOn: [Validate, BuildBinaries]` en lugar de `[BuildWindows, BuildMacOS]`.
- **GitHubRelease**: descargar artefactos del stage unificado `BuildBinaries`, cambiar `releaseNotesInline` para listar los 5 archivos (sin MSI ni PKG), y actualizar la lista `assets` para incluir los `.sha256` hermanos.

### 5.5 `H:/PROJECTS/jaavila/agm-cli/azure-pipelines-build.yml`

**Propósito actual**: pipeline CI de PR/push-to-main. Stage Check (fmt+clippy) y stage Test (Windows+macOS). Excluye paths `*.md`, `docs/**`, `*.yml`.

**Cambio**: inspección confirma que **no toca archivos de instaladores**. No requiere cambios funcionales. *Opcional*: extender `paths.exclude` para no re-ejecutarse en cambios a `scripts/install.sh` / `scripts/install.ps1` si el usuario lo prefiere — aunque el `exclude` actual sólo afecta `*.yml`, y los scripts `.sh`/`.ps1` *sí* disparan CI hoy. Recomendación: dejar como está. Los scripts de instalación deben dispararse CI (son scripts cargados con curl a producción, romperlos es severo); el test no los afecta pero tampoco es carga pesada.

### 5.6 `H:/PROJECTS/jaavila/agm-cli/scripts/install.sh`

**Propósito actual** (leído en inspección):
- Hard-codeado a `Darwin` (líneas 13-19, "this installer is for macOS only").
- Asume el archivo `agm-v${VERSION}-universal-macos.tar.gz`.
- No verifica checksum.
- Ya imprime un hint de PATH (no modifica shell profiles, bien).
- Usa `AGM_INSTALL_DIR` como override (bien).
- Usa `trap` para limpiar `TMPDIR` (bien).
- Bug menor: el `echo "Downloading agm ${VERSION} for ${TARGET}..."` en línea 34 referencia `${TARGET}` que nunca se asigna (sale vacío).

**Cambio**: reescritura completa para soportar Linux + macOS con detección de arch. Spec en sección 8.1.

### 5.7 `H:/PROJECTS/jaavila/agm-cli/scripts/install.ps1`

**Propósito actual** (leído en inspección):
- Detecta arch via `RuntimeInformation::OSArchitecture` — correcto.
- Mapea a `x86_64-pc-windows-msvc` / `aarch64-pc-windows-msvc`.
- Asume archivo `agm-v$Version-$Target.zip`.
- No verifica checksum.
- Pregunta interactivamente si añadir al PATH (`Read-Host`) y modifica el PATH User si el usuario acepta.

**Cambio**: alinear con `install.sh` — spec en sección 8.2. Decisiones clave:
- **Eliminar el prompt interactivo de PATH** y pasar a "imprime hint como starship/zoxide". El prompt rompe el idiomático `irm ... | iex` porque al pipe con `iex` no hay stdin interactivo. Aunque PowerShell *intenta* hacer `Read-Host` funcionar en pipe, es frágil y mal UX.
- **Añadir verificación de checksum con `Get-FileHash`**.
- **Soportar `AGM_VERSION` env var** para pin de versión.

### 5.8 `H:/PROJECTS/jaavila/agm-cli/scripts/build-release.sh`

**Propósito actual** (leído en inspección):
- Build local para Windows x64 (vía `cross` o `cargo` cross) + macOS universal.
- Produce `.zip` Windows, `.tar.gz` macOS universal, y `.pkg` si hay `pkgbuild` disponible.

**Decisión**: **eliminar este archivo**. Razones:
- Duplica lo que hace el pipeline CI. Si un desarrollador quiere reproducir un release localmente, `cargo build --release --target <tripleta>` + un `tar`/`zip` manual es trivial y no justifica un script de 86 líneas.
- El script menciona `.pkg`, `pkgbuild`, `universal-macos` — todo lo que estamos eliminando.
- Mantenerlo alineado con el nuevo esquema multi-target es trabajo innecesario que el pipeline ya cubre.
- No está referenciado por el CI ni por el release pipeline.

Alternativa si el usuario lo quiere conservar: rewriting minimal a `cargo build --release --target $1 && tar czf dist/agm-v$VERSION-$1.tar.gz -C target/$1/release agm`. Con ~10 líneas. Recomiendo eliminarlo directamente.

### 5.9 `H:/PROJECTS/jaavila/agm-cli/README.md`

**Propósito actual**: README principal. La sección "Installation" (líneas 9-38) cubre `cargo install`, el curl/irm a install.sh/install.ps1, y "from source".

**Cambio**: reescribir la sección Installation completa con el nuevo contenido detallado en sección 9.1. Estructura nueva:
1. `cargo install agm-cli`
2. One-liner scripts (Linux/macOS + Windows)
3. Descarga manual desde Releases (con tabla de tripletas)
4. Desde fuente
5. Tabla de plataformas soportadas
6. Verificación de checksums (instrucción breve)
7. Desinstalación por método
8. Nota sobre ausencia de firma y lo que el usuario verá (SmartScreen/Gatekeeper)

### 5.10 `H:/PROJECTS/jaavila/agm-cli/docs/releasing.md`

**Propósito actual** (leído en inspección):
- Versionado manual vía `Cargo.toml`.
- Uso de `cargo release`.
- Descripción de stages del pipeline (incluye menciones a "MSI installer + portable zip", "Universal .pkg").
- Tabla "Release Artifacts" con columnas MSI, PKG.
- Sección "Installation Methods" con prosa detallada del MSI y del .pkg (líneas 119-154).

**Cambio**: reescritura parcial de las secciones:
- Tabla "CI/CD Pipelines → Release" (líneas 76-83): reemplazar filas BuildWindows/BuildMacOS por una fila única "BuildBinaries (matrix 5 targets)".
- Diagrama ASCII (líneas 86-104): simplificar a `Validate → BuildBinaries (5 parallel jobs) → [PublishCrate | GitHubRelease]`.
- Tabla "Release Artifacts" (líneas 109-116): reemplazar por la tabla nueva con 5 filas (una por tripleta), columna "Archive" (.tar.gz / .zip), columna "Checksum" (.sha256).
- Sección "Installation Methods" (líneas 118-161): simplificar drásticamente o eliminar y delegar a README (referenciar con link). Recomendación: eliminar la sección entera y añadir una línea *"See [README.md](../README.md#installation) for end-user install instructions. This doc covers the release engineering side only."*

### 5.11 `H:/PROJECTS/jaavila/agm-cli/CONTRIBUTING.md`

**Propósito actual** (leído): setup de dev, testing, workflow PR.

**Cambio**: inspección confirma que **no menciona WiX, cargo-wix, pkgbuild, Pillow ni íconos**. No requiere cambios. *Opcional*: añadir una nota explícita de "Prerequisites" aclarando que no se necesita Pillow ni WiX ni Xcode Command Line Tools avanzados — pero esto es trivial. Recomendación: no tocar.

### 5.12 `H:/PROJECTS/jaavila/agm-cli/docs/api.md`

**Propósito actual** (leído, 100 primeras líneas y `Grep`): referencia del CLI por comando.

**Cambio**: la única mención relevante a "install" está en las líneas 921/938 del comando `agm update`, que describe `--check` / instalación desde CLI. Estas son referencias al subsistema `self_update`, que sigue apuntando a GitHub Releases tar.gz/zip — por lo que mientras el feature `self-update` siga funcionando con los nuevos nombres de archivo (ver sección 11), **`docs/api.md` no requiere cambios**. Verificar no obstante que los ejemplos de `agm update --check` siguen siendo válidos tras el primer release del nuevo esquema.

### 5.13 Otras referencias a revisar

`Grep` de los términos relevantes (`wix|cargo-wix|winresource|gen-icons|WixUI|productbuild|pkgbuild|\.icns|installer/macos`) devolvió 8 archivos — todos ya cubiertos arriba salvo `Cargo.lock`, que se regenera. Los 50 matches de `\.pkg` incluyen sidecars del *spec* AGM (`agm.mem`, `state`) que no tienen relación con el instalador `.pkg` — son falsos positivos del grep. Se confirma que no hay más lugares que editar.

## 6. Archivos nuevos a crear

Minimalismo deliberado. Solo lo imprescindible.

### 6.1 (Opcional) `H:/PROJECTS/jaavila/agm-cli/CHANGELOG.md`

**Si no existe ya**, crear una entrada para el primer release post-refactor:

```markdown
## [1.1.0] - 2026-MM-DD

### Changed

- **Distribution model rewritten.** `agm-cli` now ships as pre-built archives
  per target triple (`.tar.gz` / `.zip`) via GitHub Releases, matching the
  pattern used by ripgrep, fd, bat and other Rust CLIs. The following
  installers have been **removed**:
  - Windows MSI (`agm-vX.Y.Z-x86_64-windows.msi`)
  - macOS universal pkg (`agm-vX.Y.Z-universal-macos.pkg`)

### Added

- Pre-built binaries for Linux (`x86_64-unknown-linux-gnu`,
  `aarch64-unknown-linux-gnu`).
- SHA-256 checksum files for every release asset.
- `install.sh` now supports Linux (previously macOS-only).
- Version pinning via `AGM_VERSION=vX.Y.Z` env var in install scripts.

### Removed

- `crates/agm-cli/wix/` (WiX Toolset manifest + assets).
- `crates/agm-cli/installer/` (macOS productbuild resources).
- `crates/agm-cli/build.rs` (no longer embeds VERSIONINFO).
- `winresource` build-dependency.
- `scripts/gen-icons.py` (Pillow icon generator).
- `scripts/build-release.sh` (duplicated CI work).

### Migration notes for existing users

- If you installed via MSI on Windows: uninstall from
  **Settings → Apps → agm-cli**, then reinstall via `install.ps1` or
  `cargo install agm-cli`.
- If you installed via `.pkg` on macOS: run
  `sudo /usr/local/share/agm-cli/uninstall.sh` (still present from your
  previous install), then reinstall via `install.sh` or
  `cargo install agm-cli`.
```

Si `CHANGELOG.md` ya existe, añadir la sección al tope. Si el proyecto no usa changelog, no crearlo: basta con un buen mensaje de commit + release notes en GitHub.

### 6.2 No se crean otros archivos nuevos

Resistir la tentación de añadir `.github/workflows/release.yml`, `cross-toolchain.toml`, `Dist.toml`, `release.toml`, etc. El CI es Azure Pipelines; no migrar a GitHub Actions como parte de este cambio (sería un refactor distinto).

## 7. Arquitectura propuesta del pipeline

### 7.1 Stages nuevos

```
Validate               (1 job, ubuntu)
      │
      ▼
BuildBinaries          (5 jobs en matrix — ver 7.2)
      │
      ├───────────────────────┐
      ▼                       ▼
PublishCrate            GitHubRelease
(1 job, ubuntu)         (1 job, ubuntu)
```

- `Validate` permanece idéntico.
- `BuildBinaries` es un *único stage* con un *único job* que usa la `strategy.matrix` de Azure Pipelines para ejecutar en paralelo sobre las 5 tripletas.
- `PublishCrate` y `GitHubRelease` corren en paralelo, ambas `dependsOn: [Validate, BuildBinaries]`.

### 7.2 Matrix strategy para `BuildBinaries`

Pseudo-YAML (estructura — no copiarse literal sin revisar):

```yaml
- stage: BuildBinaries
  displayName: 'Build release binaries'
  dependsOn: Validate
  jobs:
    - job: Build
      strategy:
        matrix:
          windows-x86_64:
            vmImage: 'windows-latest'
            target: 'x86_64-pc-windows-msvc'
            archive_ext: 'zip'
            bin_name: 'agm.exe'
            use_cross: 'false'
          linux-x86_64:
            vmImage: 'ubuntu-latest'
            target: 'x86_64-unknown-linux-gnu'
            archive_ext: 'tar.gz'
            bin_name: 'agm'
            use_cross: 'false'
          linux-aarch64:
            vmImage: 'ubuntu-latest'
            target: 'aarch64-unknown-linux-gnu'
            archive_ext: 'tar.gz'
            bin_name: 'agm'
            use_cross: 'true'          # usa `cross` tool
          macos-x86_64:
            vmImage: 'macOS-latest'
            target: 'x86_64-apple-darwin'
            archive_ext: 'tar.gz'
            bin_name: 'agm'
            use_cross: 'false'
          macos-aarch64:
            vmImage: 'macOS-latest'
            target: 'aarch64-apple-darwin'
            archive_ext: 'tar.gz'
            bin_name: 'agm'
            use_cross: 'false'
      pool:
        vmImage: $(vmImage)
      steps:
        - checkout: self

        - script: |
            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
            source $HOME/.cargo/env
            rustup target add $(target)
          displayName: 'Install Rust + target'
          condition: ne(variables['Agent.OS'], 'Windows_NT')

        - powershell: |
            Invoke-WebRequest -Uri https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe -OutFile rustup-init.exe
            .\rustup-init.exe -y --default-toolchain stable
            echo "##vso[task.prependpath]$env:USERPROFILE\.cargo\bin"
            rustup target add $(target)
          displayName: 'Install Rust + target (Windows)'
          condition: eq(variables['Agent.OS'], 'Windows_NT')

        - script: |
            if [ "$(use_cross)" = "true" ]; then
              cargo install cross --git https://github.com/cross-rs/cross
              cross build --release --target $(target) -p agm-cli
            else
              cargo build --release --target $(target) -p agm-cli
            fi
          displayName: 'Build release binary'
          condition: ne(variables['Agent.OS'], 'Windows_NT')

        - script: cargo build --release --target $(target) -p agm-cli
          displayName: 'Build release binary (Windows)'
          condition: eq(variables['Agent.OS'], 'Windows_NT')

        # ── Package: stage a directory then tar/zip it ────
        - script: |
            set -e
            VERSION=$(echo "$(TAG_NAME)" | sed 's/^v//')
            STAGE="agm-v${VERSION}-$(target)"
            mkdir -p "dist/${STAGE}"
            cp "target/$(target)/release/$(bin_name)" "dist/${STAGE}/"
            cp README.md LICENSE "dist/${STAGE}/"
            cd dist
            tar czf "${STAGE}.tar.gz" "${STAGE}"
            sha256sum "${STAGE}.tar.gz" > "${STAGE}.tar.gz.sha256"
          displayName: 'Package tar.gz + sha256'
          condition: ne(variables['archive_ext'], 'zip')

        - powershell: |
            $Version = "$(TAG_NAME)" -replace '^v',''
            $Stage = "agm-v$Version-$(target)"
            New-Item -ItemType Directory -Force -Path "dist\$Stage" | Out-Null
            Copy-Item "target\$(target)\release\$(bin_name)" "dist\$Stage\"
            Copy-Item README.md, LICENSE "dist\$Stage\"
            Compress-Archive -Path "dist\$Stage\*" -DestinationPath "dist\$Stage.zip" -Force
            $hash = (Get-FileHash "dist\$Stage.zip" -Algorithm SHA256).Hash.ToLower()
            Set-Content -Path "dist\$Stage.zip.sha256" -Value "$hash  $Stage.zip"
          displayName: 'Package zip + sha256'
          condition: eq(variables['archive_ext'], 'zip')

        - publish: dist
          artifact: 'release-$(target)'
          displayName: 'Publish artifact'
```

**Notas del matrix**:
- Los `condition` por OS dentro del mismo job permiten una sola definición declarativa. Alternativa: dividir en dos jobs (unix + windows) con `strategy.matrix` más pequeño en cada uno. La versión anterior es más compacta.
- El uso de `sha256sum` funciona en Linux y macOS por defecto; en Windows se usa `Get-FileHash`.
- `cross` solo se instala cuando `use_cross=true` (solo linux-aarch64). Esto evita instalar cross en jobs que no lo necesitan.
- **Alternativa para aarch64-linux sin cross**: usar `ubuntu-latest` con `sudo apt-get install -y gcc-aarch64-linux-gnu` y setear `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc`. Más simple que `cross` pero menos hermético. Cualquiera funciona; recomendación: empezar con `cross` (más probado), cambiar a apt-gcc si se detectan problemas de tiempo de compilación.

### 7.3 Stage `PublishCrate`

Sin cambios funcionales salvo el `dependsOn`:

```yaml
- stage: PublishCrate
  displayName: 'Publish to crates.io'
  dependsOn:
    - Validate
    - BuildBinaries
  # ... resto idéntico al actual ...
```

**Nota importante**: `cargo publish` **no depende** de los binarios compilados. Podría en teoría correr en paralelo con `BuildBinaries`. Mantener el `dependsOn` solo como gating de seguridad: no publicar a crates.io si los builds fallan, evitar versiones huérfanas en crates.io sin binarios en GitHub.

### 7.4 Stage `GitHubRelease`

```yaml
- stage: GitHubRelease
  displayName: 'Create GitHub Release'
  dependsOn:
    - Validate
    - BuildBinaries
  jobs:
    - job: Release
      pool:
        vmImage: 'ubuntu-latest'
      steps:
        - download: current
          patterns: 'release-*/**'
          displayName: 'Download all release artifacts'

        - script: |
            mkdir -p release-assets
            find $(Pipeline.Workspace) -name 'agm-v*' -type f \
              -exec cp {} release-assets/ \;
            ls -la release-assets/
          displayName: 'Flatten artifacts'

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

              **Linux / macOS:**
              ```sh
              curl -fsSL https://raw.githubusercontent.com/JAAvila-Of/agm-cli/main/scripts/install.sh | sh
              ```

              **Windows (PowerShell):**
              ```powershell
              irm https://raw.githubusercontent.com/JAAvila-Of/agm-cli/main/scripts/install.ps1 | iex
              ```

              ### Manual download

              | Platform             | Archive                                                  |
              |----------------------|----------------------------------------------------------|
              | Windows x86_64       | `agm-$(TAG_NAME)-x86_64-pc-windows-msvc.zip`            |
              | Linux x86_64 (gnu)   | `agm-$(TAG_NAME)-x86_64-unknown-linux-gnu.tar.gz`       |
              | Linux aarch64 (gnu)  | `agm-$(TAG_NAME)-aarch64-unknown-linux-gnu.tar.gz`      |
              | macOS x86_64         | `agm-$(TAG_NAME)-x86_64-apple-darwin.tar.gz`            |
              | macOS aarch64        | `agm-$(TAG_NAME)-aarch64-apple-darwin.tar.gz`           |

              Each archive has a `.sha256` companion for integrity checking.
            assets: |
              release-assets/*
            isDraft: false
            isPreRelease: false
            changeLogCompareToRelease: 'lastFullRelease'
            changeLogType: 'commitBased'
          displayName: 'Create GitHub Release'
```

**Nota sobre `assets`**: el uso de glob `release-assets/*` sube tanto los archivos `.tar.gz` / `.zip` *como* sus hermanos `.sha256`. Validar en el primer dry-run que los 10 archivos (5 binarios + 5 checksums) aparecen en el release.

## 8. Especificación de los scripts de instalación

### 8.1 `scripts/install.sh` (Linux + macOS)

Spec, pseudo-código sh:

```sh
#!/bin/sh
# AGM CLI installer for Linux and macOS
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/JAAvila-Of/agm-cli/main/scripts/install.sh | sh
#   AGM_VERSION=v1.2.3 curl ... | sh
#   AGM_INSTALL_DIR=/opt/agm/bin curl ... | sh

set -e

REPO="JAAvila-Of/agm-cli"
BIN_NAME="agm"
INSTALL_DIR="${AGM_INSTALL_DIR:-$HOME/.local/bin}"

# --- Detect OS + arch -----------------------------------------------------
uname_s="$(uname -s)"
uname_m="$(uname -m)"

case "$uname_s" in
    Linux)
        case "$uname_m" in
            x86_64 | amd64)  TARGET="x86_64-unknown-linux-gnu"  ;;
            aarch64 | arm64) TARGET="aarch64-unknown-linux-gnu" ;;
            *) echo "error: unsupported Linux arch: $uname_m" >&2; exit 1 ;;
        esac
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
        echo "       supported: Linux, Darwin (macOS)" >&2
        echo "       on Windows, use install.ps1" >&2
        exit 1
        ;;
esac

# --- Resolve version ------------------------------------------------------
if [ -n "${AGM_VERSION:-}" ]; then
    VERSION_TAG="$AGM_VERSION"
else
    echo "Fetching latest release..."
    VERSION_TAG=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' \
        | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/')
fi

if [ -z "$VERSION_TAG" ]; then
    echo "error: could not determine version to install" >&2
    exit 1
fi

# --- Download + verify ----------------------------------------------------
ARCHIVE="agm-${VERSION_TAG}-${TARGET}.tar.gz"
URL="https://github.com/${REPO}/releases/download/${VERSION_TAG}/${ARCHIVE}"
SHA_URL="${URL}.sha256"

echo "Downloading ${ARCHIVE}..."
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

curl -fsSL "$URL"     -o "${TMPDIR}/${ARCHIVE}"
curl -fsSL "$SHA_URL" -o "${TMPDIR}/${ARCHIVE}.sha256"

echo "Verifying checksum..."
cd "$TMPDIR"
if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c "${ARCHIVE}.sha256" || {
        echo "error: checksum verification failed" >&2
        exit 1
    }
elif command -v shasum >/dev/null 2>&1; then
    # macOS default
    EXPECTED=$(awk '{print $1}' "${ARCHIVE}.sha256")
    ACTUAL=$(shasum -a 256 "${ARCHIVE}" | awk '{print $1}')
    [ "$EXPECTED" = "$ACTUAL" ] || {
        echo "error: checksum mismatch" >&2
        exit 1
    }
else
    echo "warning: no sha256sum/shasum available, skipping checksum verify" >&2
fi
cd - >/dev/null

# --- Extract + install ----------------------------------------------------
echo "Extracting..."
tar -xzf "${TMPDIR}/${ARCHIVE}" -C "$TMPDIR"

echo "Installing to ${INSTALL_DIR}..."
mkdir -p "$INSTALL_DIR"
STAGE_DIR="${TMPDIR}/agm-${VERSION_TAG}-${TARGET}"
mv "${STAGE_DIR}/${BIN_NAME}" "${INSTALL_DIR}/${BIN_NAME}"
chmod 755 "${INSTALL_DIR}/${BIN_NAME}"

echo ""
echo "Installed agm ${VERSION_TAG} to ${INSTALL_DIR}/${BIN_NAME}"

# --- PATH hint ------------------------------------------------------------
case ":$PATH:" in
    *":${INSTALL_DIR}:"*)
        echo "Run: agm --version"
        ;;
    *)
        echo ""
        echo "Note: ${INSTALL_DIR} is not in your PATH."
        echo "Add it by appending the following line to your shell profile"
        echo "(~/.bashrc, ~/.zshrc, ~/.profile, etc.):"
        echo ""
        echo "    export PATH=\"${INSTALL_DIR}:\$PATH\""
        ;;
esac
```

**Puntos a destacar sobre el spec**:
- `#!/bin/sh` + `set -e` (no `set -euo pipefail`): mantener POSIX, compatible con BusyBox ash si alguien lo corre en Alpine (preparando futura distro musl).
- No auto-modifica `~/.bashrc`: decisión explícita alineada con starship/zoxide. Muy deliberada por respeto al shell profile del usuario.
- Verificación de checksum *es* obligatoria si hay tooling (`sha256sum` o `shasum`); si no hay, emite warning y continúa (mejor instalar que bloquear en un sistema roto).
- `AGM_VERSION=vX.Y.Z` permite pinnear. `AGM_INSTALL_DIR` permite redirigir destino.
- `$STAGE_DIR` extrae al subdirectorio que el tar contiene (sección 3.3).
- Nunca intenta `sudo`. Install a `$HOME/.local/bin` por defecto (XDG-compliant).

### 8.2 `scripts/install.ps1` (Windows)

Spec, pseudo-código PowerShell:

```powershell
# AGM CLI installer for Windows
# Usage:
#   irm https://raw.githubusercontent.com/JAAvila-Of/agm-cli/main/scripts/install.ps1 | iex
#   $env:AGM_VERSION = 'v1.2.3'; irm ... | iex
#   $env:AGM_INSTALL_DIR = 'C:\tools\agm'; irm ... | iex

$ErrorActionPreference = "Stop"

$Repo       = "JAAvila-Of/agm-cli"
$BinName    = "agm.exe"
$InstallDir = if ($env:AGM_INSTALL_DIR) {
    $env:AGM_INSTALL_DIR
} else {
    Join-Path $env:LOCALAPPDATA "Programs\agm\bin"
}

# --- Detect arch ----------------------------------------------------------
$Arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
switch ($Arch) {
    "X64"   { $Target = "x86_64-pc-windows-msvc" }
    "Arm64" {
        Write-Warning "aarch64-pc-windows-msvc is not currently published. Aborting."
        Write-Warning "Install via: cargo install agm-cli"
        exit 1
    }
    default {
        Write-Error "Unsupported Windows architecture: $Arch"
        exit 1
    }
}

# --- Resolve version ------------------------------------------------------
if ($env:AGM_VERSION) {
    $VersionTag = $env:AGM_VERSION
} else {
    Write-Host "Fetching latest release..."
    $Release    = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
    $VersionTag = $Release.tag_name
}

if (-not $VersionTag) {
    Write-Error "Could not determine version to install"
    exit 1
}

# --- Download + verify ----------------------------------------------------
$Archive = "agm-$VersionTag-$Target.zip"
$Url     = "https://github.com/$Repo/releases/download/$VersionTag/$Archive"
$ShaUrl  = "$Url.sha256"

Write-Host "Downloading $Archive..."
$TmpDir = New-Item -ItemType Directory -Path (Join-Path $env:TEMP "agm-install-$(Get-Random)")

try {
    $ArchivePath = Join-Path $TmpDir $Archive
    $ShaPath     = "$ArchivePath.sha256"

    Invoke-WebRequest -Uri $Url    -OutFile $ArchivePath
    Invoke-WebRequest -Uri $ShaUrl -OutFile $ShaPath

    Write-Host "Verifying checksum..."
    $Expected = (Get-Content $ShaPath).Split()[0].ToLower()
    $Actual   = (Get-FileHash $ArchivePath -Algorithm SHA256).Hash.ToLower()
    if ($Expected -ne $Actual) {
        Write-Error "Checksum mismatch: expected $Expected, got $Actual"
        exit 1
    }

    Write-Host "Extracting..."
    Expand-Archive -Path $ArchivePath -DestinationPath $TmpDir -Force

    $StageDir = Join-Path $TmpDir "agm-$VersionTag-$Target"

    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }

    Write-Host "Installing to $InstallDir..."
    Copy-Item (Join-Path $StageDir $BinName) (Join-Path $InstallDir $BinName) -Force

    Write-Host ""
    Write-Host "Installed agm $VersionTag to $InstallDir\$BinName"

    # --- PATH hint ------------------------------------------------------
    $UserPath = [Environment]::GetEnvironmentVariable("PATH", "User")
    if ($UserPath -notlike "*$InstallDir*") {
        Write-Host ""
        Write-Host "Note: $InstallDir is not in your PATH."
        Write-Host "Add it by running (one-time):"
        Write-Host ""
        Write-Host "    [Environment]::SetEnvironmentVariable('PATH', `"`$env:PATH;$InstallDir`", 'User')"
        Write-Host ""
        Write-Host "Then restart your terminal."
    } else {
        Write-Host "Run: agm --version"
    }
}
finally {
    Remove-Item -Recurse -Force $TmpDir -ErrorAction SilentlyContinue
}
```

**Decisiones clave del spec**:
- **Destino**: `$env:LOCALAPPDATA\Programs\agm\bin` (no `$HOME\.local\bin`). Razón: `%LOCALAPPDATA%\Programs` es la convención que usa scoop y los instaladores user-scope de Microsoft Store; `.local/bin` es Unix-ish y pocos usuarios Windows lo tienen en PATH.
- **No prompt interactivo**: elimina el `Read-Host` actual que pregunta si añadir al PATH. Pipea a `iex` es no-interactivo por definición; el hint mostrado es autoaplicable.
- **Detección de Arm64**: reporta y aborta, no intenta descargar un archivo que no existe. Reubicar a `cargo install` como fallback.
- **Checksum con `Get-FileHash`**: nativo PowerShell, sin dependencias.
- **Limpieza con `try/finally`**: garantiza el `rm -rf` del tmpdir incluso si falla el middle.
- **`AGM_VERSION` y `AGM_INSTALL_DIR` vía env var**: paridad con el `install.sh`.

## 9. Cambios de documentación

### 9.1 `README.md` — nueva sección Installation (contenido completo a copiar)

```markdown
## Installation

### Rust users (recommended)

```bash
cargo install agm-cli
```

### One-line installers

**Linux / macOS:**
```bash
curl -fsSL https://raw.githubusercontent.com/JAAvila-Of/agm-cli/main/scripts/install.sh | sh
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/JAAvila-Of/agm-cli/main/scripts/install.ps1 | iex
```

Pin a specific version:

```bash
AGM_VERSION=v1.0.0 curl -fsSL https://raw.githubusercontent.com/JAAvila-Of/agm-cli/main/scripts/install.sh | sh
```

```powershell
$env:AGM_VERSION = 'v1.0.0'; irm https://raw.githubusercontent.com/JAAvila-Of/agm-cli/main/scripts/install.ps1 | iex
```

### Manual download

Grab the archive for your platform from
[GitHub Releases](https://github.com/JAAvila-Of/agm-cli/releases),
extract the `agm` binary, and place it somewhere on your `PATH`.

Supported platforms:

| OS       | Architecture | Archive                                             |
|----------|--------------|-----------------------------------------------------|
| Windows  | x86_64       | `agm-vX.Y.Z-x86_64-pc-windows-msvc.zip`             |
| Linux    | x86_64       | `agm-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz`        |
| Linux    | aarch64      | `agm-vX.Y.Z-aarch64-unknown-linux-gnu.tar.gz`       |
| macOS    | x86_64       | `agm-vX.Y.Z-x86_64-apple-darwin.tar.gz`             |
| macOS    | aarch64      | `agm-vX.Y.Z-aarch64-apple-darwin.tar.gz`            |

Each archive ships with a `.sha256` companion. To verify:

```bash
# Linux / macOS
sha256sum -c agm-vX.Y.Z-<target>.tar.gz.sha256
```

```powershell
# Windows
Get-FileHash agm-vX.Y.Z-x86_64-pc-windows-msvc.zip -Algorithm SHA256
```

### From source

```bash
git clone https://github.com/JAAvila-Of/agm-cli.git
cd agm-cli
cargo build --release
# Binary at target/release/agm (or agm.exe on Windows)
```

### Uninstall

| Install method        | Uninstall                                            |
|-----------------------|------------------------------------------------------|
| `cargo install`       | `cargo uninstall agm-cli`                            |
| `install.sh`          | `rm $HOME/.local/bin/agm`                            |
| `install.ps1`         | `Remove-Item "$env:LOCALAPPDATA\Programs\agm\bin\agm.exe"` |
| Manual                | Delete the `agm` binary from its location            |

### Security notes

`agm` binaries are not currently code-signed. On Windows, SmartScreen may
warn when running an unsigned `.exe` downloaded from the internet; the
one-line installer drops the binary directly into user-scope and does not
trigger SmartScreen. On macOS, Gatekeeper only applies to `.app` bundles
and `.pkg` installers, so the raw `agm` binary extracted from a `.tar.gz`
runs unimpeded.

If you prefer a signed build, install via `cargo install agm-cli` — you
build it yourself from verified source on crates.io.
```

### 9.2 `docs/releasing.md` — cambios resumidos

- **Tabla de Release stage** (líneas 76-83): reemplazar filas `BuildWindows` + `BuildMacOS` por una sola fila `BuildBinaries | Matrix build across 5 target triples (tar.gz / zip + sha256)`.
- **Diagrama ASCII** (líneas 86-104): reemplazar por:

```
push to main/PR ──→ CI Build (lint → test)
                        │
                      merge
                        │
git tag v0.1.0 ────→ Release Pipeline
                        │
                   Validate tag
                        │
                   BuildBinaries
            (matrix: 5 target triples in parallel)
                        │
              ┌─────────┴─────────┐
         PublishCrate        GitHub Release
         (cargo publish)     (upload 5 archives
                              + 5 .sha256 files)
              └───────────────────┘
```

- **Tabla "Release Artifacts"** (líneas 109-116): reemplazar por:

```markdown
| Artifact | Platform | Notes |
|----------|----------|-------|
| `agm-vX.Y.Z-x86_64-pc-windows-msvc.zip`         | Windows x86_64  | + `.sha256` |
| `agm-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz`    | Linux x86_64    | + `.sha256` |
| `agm-vX.Y.Z-aarch64-unknown-linux-gnu.tar.gz`   | Linux aarch64   | + `.sha256` |
| `agm-vX.Y.Z-x86_64-apple-darwin.tar.gz`         | macOS x86_64    | + `.sha256` |
| `agm-vX.Y.Z-aarch64-apple-darwin.tar.gz`        | macOS aarch64   | + `.sha256` |
```

- **Sección "Installation Methods"** (líneas 118-161): **eliminar por completo**, reemplazar con:

```markdown
## End-user installation

See [README.md → Installation](../README.md#installation). This document
covers release engineering; user-facing install instructions live in the
README.
```

### 9.3 `CONTRIBUTING.md` — sin cambios

Verificado en inspección: no menciona WiX/Pillow/pkg. No tocar.

### 9.4 `docs/api.md` — sin cambios

Verificado: las únicas menciones a "install" refieren al subcomando `agm update`, cuya operación interna (descargar archive de GitHub Releases) sigue funcionando mientras los nombres de archivo sean compatibles con `self_update`. Ver sección 11 sobre esa compatibilidad.

## 10. Orden de implementación

Secuencial, cada paso ejecutable en una sesión enfocada y verificable de forma independiente.

### Task 1 — Eliminar directorios de instaladores

- **Archivos tocados**: `crates/agm-cli/wix/` (recursivo), `crates/agm-cli/installer/` (recursivo).
- **Comando tipo**: `git rm -r crates/agm-cli/wix crates/agm-cli/installer`.
- **Verificación**: `git status` muestra deleciones; `Glob crates/agm-cli/wix/**` devuelve vacío; `Glob crates/agm-cli/installer/**` devuelve vacío.
- **Rollback**: `git restore --staged --worktree crates/agm-cli/wix crates/agm-cli/installer`.

### Task 2 — Eliminar `build.rs` y `winresource`

- **Archivos tocados**: `crates/agm-cli/build.rs` (delete), `crates/agm-cli/Cargo.toml` (remover líneas 40-43), `Cargo.lock` (regeneración automática).
- **Comando tipo**:
  1. `git rm crates/agm-cli/build.rs`
  2. Editar `crates/agm-cli/Cargo.toml` — eliminar las 4 líneas de `[target.'cfg(windows)'.build-dependencies]` y su comentario.
  3. `cargo check -p agm-cli` — debe regenerar `Cargo.lock` sin `winresource`.
- **Verificación**:
  - `cargo metadata --no-deps --format-version 1 | grep winresource` → sin resultados.
  - `cargo build --release -p agm-cli` en Windows → produce `target/release/agm.exe`. El binario ya no tendrá icono propio ni VERSIONINFO custom (Windows lo marcará como "agm.exe, Microsoft Corporation" por defecto del linker — estéticamente distinto pero funcionalmente idéntico).
  - `cargo build` en Linux/macOS → sin cambios.
- **Rollback**: `git restore crates/agm-cli/build.rs crates/agm-cli/Cargo.toml Cargo.lock`.

### Task 3 — Correr tests locales y de CI completos

- **Propósito**: asegurar que Task 1 + Task 2 no rompieron nada.
- **Comandos**: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- **En CI**: empujar a una rama `refactor/remove-installers`, verificar que `azure-pipelines-build.yml` (CI) pasa en Windows + macOS. El pipeline de release no se dispara (solo disparador en tag).
- **Verificación**: todos los jobs en verde.
- **Rollback**: `git reset --hard HEAD~N` la rama.

### Task 4 — Eliminar `scripts/gen-icons.py` y `scripts/build-release.sh`

- **Archivos tocados**: `scripts/gen-icons.py`, `scripts/build-release.sh`.
- **Comando tipo**: `git rm scripts/gen-icons.py scripts/build-release.sh`.
- **Verificación**: `Grep "gen-icons\|build-release"` en todo el repo → sin referencias. `Grep "Pillow\|PIL"` → sin referencias.
- **Rollback**: `git restore --staged --worktree scripts/`.

### Task 5 — Reescribir `scripts/install.sh`

- **Archivo tocado**: `scripts/install.sh` (reemplazo completo).
- **Fuente**: el spec en la sección 8.1.
- **Verificación local** (antes de mergear): apuntar `REPO` a un fork o usar `AGM_VERSION` con un tag existente del repo:

  ```bash
  # Probar contra el tag actual (asume aún existe el archivo old-format)
  # O probar contra un pre-release de prueba si ya se hizo uno.
  AGM_VERSION=v1.0.0 AGM_INSTALL_DIR=/tmp/agm-test sh scripts/install.sh
  /tmp/agm-test/agm --version
  ```

  Nota: esto **no** funcionará hasta que exista un release con el nuevo esquema de assets. Ver Task 8.
- **Rollback**: `git restore scripts/install.sh`.

### Task 6 — Reescribir `scripts/install.ps1`

- **Archivo tocado**: `scripts/install.ps1` (reemplazo completo).
- **Fuente**: el spec en la sección 8.2.
- **Verificación local**: igual que Task 5, mismo caveat.

### Task 7 — Reescribir `azure-pipelines-deploy.yml`

- **Archivo tocado**: `azure-pipelines-deploy.yml` (reemplazo de stages 3-6, Validate y PublishCrate preservados).
- **Fuente**: la estructura detallada en sección 7.
- **Verificación**: no hay forma de verificar sin correr el pipeline. Opciones:
  - **Opción A (conservadora)**: crear un tag de pre-release `v1.1.0-rc.1` que dispare el pipeline; si falla, ajustar e iterar.
  - **Opción B (agresiva)**: tagear directamente `v1.1.0`. Si falla, ya publicó parcialmente (crates.io es inmutable).
  - **Recomendación: Opción A**.
- **Verificaciones post-run**:
  1. Stage `BuildBinaries` termina los 5 jobs en verde.
  2. Los 10 artifacts (5 .tar.gz/.zip + 5 .sha256) aparecen en el GitHub Release.
  3. Descargar uno, `sha256sum -c`, extraer, ejecutar `./agm --version`.
  4. `cargo publish` stage tiene éxito.
  5. `crates.io/crates/agm-cli` muestra la nueva versión.
- **Rollback**: si falla, `git revert` el commit del pipeline. Si `cargo publish` ya corrió: yank (`cargo yank --vers X.Y.Z`) y re-release con una versión patch arriba.

### Task 8 — Actualizar README y docs/releasing.md

- **Archivos tocados**: `README.md`, `docs/releasing.md`.
- **Fuente**: secciones 9.1 y 9.2 de este plan.
- **Verificación**: render Markdown en GitHub; todos los enlaces resolven; la tabla de tripletas coincide con la matrix del pipeline.
- **Rollback**: `git restore README.md docs/releasing.md`.

### Task 9 — Decidir sobre `logo.png`

- **Opción A**: `git add logo.png` — añadirlo al commit y dejarlo como asset de branding.
- **Opción B**: dejarlo untracked (estado actual) — efectivamente lo ignora sin añadirlo al repo.
- **Opción C**: `rm logo.png` — eliminarlo del filesystem si se decide no usarlo.
- **Recomendación**: Opción A. Añadirlo al repo para branding futuro.

### Task 10 — Tag + release de verdad

- **Pasos**:
  1. `cargo release patch --execute` o bumping manual + tag.
  2. `git push --tags`.
  3. Monitorear el pipeline.
  4. Validar end-to-end con el `install.sh` y `install.ps1` en máquinas reales de las 5 plataformas (ver sección 12).
- **Rollback**: yank del crate si hay bug; hotfix patch; re-release.

### Task 11 — (Opcional) Añadir `CHANGELOG.md` entry

- Si se adopta la sección 6.1.

## 11. Análisis de riesgos

### 11.1 Usuarios existentes con MSI / .pkg instalado

**Riesgo**: un usuario con MSI v1.0.0 actualiza vía `agm update` (self_update), y ahora los archivos descargables cambian de nombre.

**Impacto**: dependerá de cómo el feature `self-update` está configurado en `crates/agm-cli`. El crate `self_update = "0.42"` con features `archive-zip, archive-tar, compression-zip-deflate` soporta perfectamente los nuevos tarballs/zips — **siempre que el código en `src/` que invoca `self_update` construya el nombre de archivo con la tripleta correcta**. Este es un punto crítico que **debe verificarse** antes del merge:

- Inspeccionar `crates/agm-cli/src/**/update*.rs` (o donde viva el comando `agm update`) y confirmar que el esquema de nombre de archivo que espera coincide con `agm-v{version}-{target}.tar.gz` / `.zip`.
- Si hoy el código espera `agm-v{version}-universal-macos.pkg` o `...-x86_64-windows.msi`, el comando `agm update` romperá al primer release post-refactor.

**Mitigación**: añadir Task 7.5 intermedio — auditar `src/commands/update.rs` (o equivalente) y adaptar el template de nombre de archivo. Este es un hallazgo del que depende que `agm update` siga funcionando; no es opcional.

**Mitigación adicional para usuarios de MSI**: documentar en README y en las release notes del primer release post-refactor las instrucciones exactas:

```
Windows MSI users:
  1. Settings → Apps → agm-cli → Uninstall
  2. irm https://raw.githubusercontent.com/.../install.ps1 | iex

macOS .pkg users:
  1. sudo /usr/local/share/agm-cli/uninstall.sh
  2. curl -fsSL https://raw.githubusercontent.com/.../install.sh | sh
```

### 11.2 Forks o PRs abiertos que referencian `wix/` o `installer/`

**Riesgo**: un fork tiene cambios locales sobre archivos que estamos eliminando.

**Impacto**: conflictos de merge cuando el fork intente rebase contra main.

**Mitigación**: anunciar el cambio en el PR grande del refactor (título claro: "Remove MSI/PKG installers, adopt pre-built archive distribution"). Con un solo fork activo hoy (asumible para un proyecto pre-1.x), bajo riesgo.

### 11.3 Cross-compile de `aarch64-unknown-linux-gnu`

**Riesgo**: primer build en CI de un target que nunca se ha construido.

**Problemas potenciales**:
- Dependencias C transitivas (si aparecen) pueden no tener versión aarch64 en el toolchain.
- `cross` requiere Docker en el runner — `ubuntu-latest` de Azure lo tiene, pero el overhead de imagen puede ser notable en el primer build (~5 min de pull).
- Si alguna dependencia usa `build.rs` con `cc` para compilar C, necesita `aarch64-linux-gnu-gcc` disponible.

**Mitigación**:
- Auditar workspace: `cargo tree -p agm-cli` y verificar que no hay crates C (como `openssl-sys`, `ring` en ciertas configuraciones). `self_update` con `archive-zip,archive-tar` usa `zip` y `tar` puros Rust, sin C.
- Si emerge un blocker, *diferir* aarch64-linux al segundo release. Quita presión sin romper el patrón.

### 11.4 `cargo install` users unaffected

**Verificación requerida**: `cargo install agm-cli` solo toca `crates.io`, descarga source, compila local. **No depende** de nada del repo de installer infra. **Riesgo: cero.** Confirmable con `cargo install agm-cli --version X.Y.Z --locked` en cualquier máquina tras el PublishCrate del primer release.

### 11.5 Pinning de `install.sh` servido desde `main`

**Riesgo**: un usuario que copió el comando curl en su memoria muscular lo ejecuta 6 meses después. Durante esos 6 meses el script cambió su lógica, el esquema de assets cambió, etc. El script que se ejecuta es *siempre* el último en `main`.

**Mitigación estructural**: el script **siempre** resuelve "latest release" vía GitHub API (o `AGM_VERSION` si el usuario pinea). Nunca hardcodea assumptions sobre qué tags existen. Así, el contrato es: "el `install.sh` en `main` sabe cómo instalar el último release al que apunta GitHub Releases/latest". Si el esquema de assets cambia *otra vez*, es responsabilidad del refactor futuro mantener el script actualizado.

**Mitigación adicional**: nunca borrar un release publicado. El pin `AGM_VERSION=v1.0.0` debe seguir funcionando para siempre.

### 11.6 Pérdida de VERSIONINFO en `agm.exe`

**Riesgo**: Windows marcará `agm.exe` sin metadata como "unknown publisher", algunas herramientas corporativas (EDR, AppLocker) pueden bloquearlo.

**Probabilidad**: baja para un CLI dev-targeted. ripgrep lo hace así y no ha sido una queja.

**Mitigación**: si aparece demanda, reintroducir `build.rs` + `winresource` es un PR de 30 líneas; nada de este plan impide hacerlo más adelante. Y la vía "correcta" para evitar warning de unknown publisher es firma (Authenticode) que está explícitamente fuera de alcance.

### 11.7 Fallo en el primer pipeline completo

**Riesgo**: tag `v1.1.0-rc.1` dispara pipeline, `BuildBinaries` matrix falla en 1 de 5 jobs, `PublishCrate` no corre (bien), pero parte de los artifacts ya están publicados en el Release de GitHub.

**Mitigación**:
- Usar `isDraft: true` en la task `GitHubRelease@1` durante los primeros rc, así el release queda en estado draft y puede eliminarse sin exponerlo públicamente.
- Cambiar a `isDraft: false` sólo cuando todo el pipeline verde y la verificación manual del archivo de cada plataforma haya pasado.

## 12. Plan de verificación y testing

### 12.1 Verificación del pipeline (pre-release)

1. Pushear commit del refactor a rama `refactor/distribution-model`.
2. CI build pipeline (azure-pipelines-build.yml) debe pasar en Windows y macOS.
3. Mergear a main.
4. Crear pre-release tag: `git tag v1.1.0-rc.1 && git push --tags`.
5. Monitorear pipeline de deploy:
   - `Validate` verde.
   - `BuildBinaries` 5 jobs verdes (esto verifica la matrix).
   - `PublishCrate` — ⚠️ **para rc, considerar skipear o usar un registry alternativo**. cargo-release permite `--no-publish`. Alternativa: mantener y publicar `1.1.0-rc.1` a crates.io (válido semver).
   - `GitHubRelease` con `isDraft: true`.
6. En GitHub UI, verificar que el release draft tiene 10 archivos esperados:
   ```
   agm-v1.1.0-rc.1-x86_64-pc-windows-msvc.zip          + .sha256
   agm-v1.1.0-rc.1-x86_64-unknown-linux-gnu.tar.gz     + .sha256
   agm-v1.1.0-rc.1-aarch64-unknown-linux-gnu.tar.gz    + .sha256
   agm-v1.1.0-rc.1-x86_64-apple-darwin.tar.gz          + .sha256
   agm-v1.1.0-rc.1-aarch64-apple-darwin.tar.gz         + .sha256
   ```
7. Descargar cada uno, verificar:

   ```bash
   sha256sum -c agm-v1.1.0-rc.1-x86_64-unknown-linux-gnu.tar.gz.sha256
   tar -tzf agm-v1.1.0-rc.1-x86_64-unknown-linux-gnu.tar.gz | head
   # Expect:
   #   agm-v1.1.0-rc.1-x86_64-unknown-linux-gnu/
   #   agm-v1.1.0-rc.1-x86_64-unknown-linux-gnu/agm
   #   agm-v1.1.0-rc.1-x86_64-unknown-linux-gnu/README.md
   #   agm-v1.1.0-rc.1-x86_64-unknown-linux-gnu/LICENSE
   ```

8. Extraer, `./agm --version` debe imprimir `1.1.0-rc.1`.

### 12.2 Test de install.sh sin release real

Imposible probar install.sh con un release fake en GitHub — el script golpea `api.github.com`. Opciones:

- **Opción A**: crear el release rc como draft, luego promover a pre-release no-draft, luego probar install.sh con `AGM_VERSION=v1.1.0-rc.1`. Tras probar, eliminar el pre-release si se quiere mantener la historia limpia (GitHub permite eliminar releases, aunque el tag queda).
- **Opción B**: servir el install.sh desde un fork local, apuntando `REPO` al fork. Más ficción que valor.
- **Recomendación: Opción A**.

Test concreto por plataforma:

```bash
# Linux x86_64 (Ubuntu 22.04 container)
AGM_VERSION=v1.1.0-rc.1 sh /path/to/install.sh
~/.local/bin/agm --version

# macOS arm64 (Apple Silicon)
AGM_VERSION=v1.1.0-rc.1 sh /path/to/install.sh
~/.local/bin/agm --version

# Windows x86_64 (PowerShell)
$env:AGM_VERSION = 'v1.1.0-rc.1'
& /path/to/install.ps1
& "$env:LOCALAPPDATA\Programs\agm\bin\agm.exe" --version
```

### 12.3 Criterios de aceptación

El refactor se considera completo y verde cuando:

1. ✅ `cargo install agm-cli` funciona en Linux, macOS, Windows, instalando `1.1.0` desde crates.io.
2. ✅ `curl ... install.sh | sh` funciona en Linux x86_64, Linux aarch64 (si se incluye desde el día uno), macOS x86_64, macOS aarch64. Resulta en un `agm` funcional en `$HOME/.local/bin/agm`.
3. ✅ `irm ... install.ps1 | iex` funciona en Windows 11 x86_64. Resulta en `agm.exe` funcional en `$env:LOCALAPPDATA\Programs\agm\bin\agm.exe`.
4. ✅ Cada archivo en el release tiene un `.sha256` hermano y la verificación de checksum pasa.
5. ✅ `agm update` sigue funcionando (descarga el tar.gz/zip del release, lo extrae, reemplaza el binario). Requiere la auditoría de la sección 11.1.
6. ✅ CI `azure-pipelines-build.yml` verde tras el merge (fmt, clippy, test en Windows+macOS).
7. ✅ Release pipeline `azure-pipelines-deploy.yml` verde end-to-end.
8. ✅ `Grep` de `wix|cargo-wix|winresource|gen-icons|productbuild|pkgbuild|WixUI|\.icns|installer/macos` en `H:/PROJECTS/jaavila/agm-cli` fuera de `Cargo.lock` (si aún no regenerado) y `tests/fixtures/**` devuelve cero matches.
9. ✅ `crates.io/crates/agm-cli` muestra versión nueva, `docs.rs/agm-core` rebuilds OK.
10. ✅ README.md y docs/releasing.md reflejan el nuevo esquema.

## 13. Tracks de seguimiento (fuera de alcance)

### 13.1 Homebrew tap

Crear repo `JAAvila-Of/homebrew-agm` con fórmula:

```ruby
class Agm < Formula
  desc "CLI for parsing, validating, and orchestrating AGM files"
  homepage "https://github.com/JAAvila-Of/agm-cli"
  version "X.Y.Z"
  license "Apache-2.0"

  on_macos do
    on_arm do
      url "https://github.com/JAAvila-Of/agm-cli/releases/download/vX.Y.Z/agm-vX.Y.Z-aarch64-apple-darwin.tar.gz"
      sha256 "..."
    end
    on_intel do
      url "https://github.com/JAAvila-Of/agm-cli/releases/download/vX.Y.Z/agm-vX.Y.Z-x86_64-apple-darwin.tar.gz"
      sha256 "..."
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/JAAvila-Of/agm-cli/releases/download/vX.Y.Z/agm-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "..."
    end
  end

  def install
    bin.install "agm"
  end
end
```

Automatizar con un job adicional en el pipeline (o GitHub Action separado) que abra PR al tap con los sha256 del release. Ver [ripgrep homebrew-core PR workflow](https://github.com/BurntSushi/ripgrep/blob/master/.github/workflows/release.yml) como referencia.

### 13.2 Scoop bucket + winget manifest

Scoop: crear `JAAvila-Of/scoop-agm` con manifest JSON apuntando al zip Windows. Similar pattern.

Winget: submit manifest al repo oficial [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs). Hay herramientas (`wingetcreate`) para automatizarlo. Requiere checksum del `.exe`, no del `.zip` — lo cual implica publicar también el `.exe` pelado o cambiar la forma en que se genera el zip.

### 13.3 Code signing

Deferido por coste. Referencia del análisis previo: USD 250-900/año dependiendo del vendor (Sectigo, DigiCert, SSL.com). Para macOS se necesita Apple Developer Program (USD 99/año) + notarización via `notarytool`. Beneficio principal: eliminar warnings SmartScreen / Gatekeeper, ganar confianza enterprise. Costo: mantenimiento del HSM o token físico, rotación de certificados, integración CI con secrets del cert. Recomendado posponer hasta demanda real de usuarios enterprise.

### 13.4 Paquetes nativos Linux

- `.deb` via `cargo-deb`. Trivial de añadir, un stage más con `cargo deb -p agm-cli`.
- `.rpm` via `cargo-generate-rpm`. Similar.
- AUR (`PKGBUILD` en `aur.archlinux.org`): community-maintained, no requiere infra propia.

Bajo costo, medio valor. Buen candidato para el segundo release post-refactor.

### 13.5 musl builds

Añadir `x86_64-unknown-linux-musl` al matrix. Produce binario estático corriendo en Alpine, distroless, busybox. Un row más en la matrix del pipeline, target trivial en `cross`. Cero costo extra si ya se usa `cross` para aarch64. Recomendado junto al segundo release.

### 13.6 Completions + man pages

La mayoría de CLIs Rust ship:
- Shell completions (`bash`, `zsh`, `fish`, `powershell`, `elvish`) generadas por clap via `clap_complete`.
- Man page generada via `clap_mangen`.

Se emiten en build-time con un `build.rs` o en run-time con un subcomando `agm completions <shell>`. Si hoy `agm-cli` no los ship, es trabajo independiente (no requiere tocar el pipeline de distribución más que añadir esos archivos al tarball).