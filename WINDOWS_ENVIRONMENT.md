# Windows Environment Setup

Build and run the Voronoi Reactive Shader as an FFGL plugin for Resolume on Windows, using WSL2 as the development environment.

## Architecture

The project lives in WSL2 (Ubuntu). The Windows host provides:
- **Rust toolchain** (via scoop/rustup) — compiles the FFGL `.dll`
- **LLVM/Clang** (via scoop) — required by `bindgen` for C/C++ header parsing
- **VS Build Tools** — provides MSVC linker and Windows SDK headers
- **Resolume Avenue** — loads the plugin at runtime

WSL2 provides:
- **Node.js** — runs the browser preview harness
- **Git + gh** — source control, submodule management

The Windows Rust toolchain builds directly against the WSL filesystem via UNC path mapping
(`\\wsl$\Ubuntu\...` → `Z:` drive via `pushd`). The build is invoked from WSL via `cmd.exe /c`,
so you never need to leave your WSL shell.

## Prerequisites

### Windows Host

1. **Scoop** (package manager — no admin required):
   ```powershell
   irm get.scoop.sh | iex
   ```

2. **Rustup via scoop:**
   ```powershell
   scoop install rustup
   # Open a fresh shell, then:
   rustup toolchain install stable-x86_64-pc-windows-msvc
   rustup default stable
   ```
   Note: always use `rustup` not the bare `rust` scoop package — rustup handles toolchain management.

3. **LLVM/Clang via scoop** (bindgen dependency):
   ```powershell
   scoop install llvm
   ```

4. **Visual Studio Build Tools** with C++ workload (provides MSVC linker and Windows SDK):
   ```powershell
   winget install Microsoft.VisualStudio.2022.BuildTools --override "--quiet --add Microsoft.VisualStudio.Workload.VCTools --add Microsoft.VisualStudio.Component.Windows11SDK.22621 --includeRecommended"
   ```
   The `VsDevCmd.bat` will be installed at:
   ```
   C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat
   ```

### WSL2 (Ubuntu)

Keep all project files in the WSL filesystem (`~/dev/`), not on `/mnt/d` or `/mnt/c`. Cross-filesystem
I/O is significantly slower for git operations and Rust builds.

1. **gh CLI** (use the official apt repo — the apt default is outdated, snap is unreliable in WSL):
   ```bash
   (type -p wget >/dev/null || (sudo apt-get update && sudo apt-get install wget -y)) \
   && sudo mkdir -p -m 755 /etc/apt/keyrings \
   && wget -qO- https://cli.github.com/packages/githubcli-archive-keyring.gpg | sudo tee /etc/apt/keyrings/githubcli-archive-keyring.gpg > /dev/null \
   && sudo chmod go+r /etc/apt/keyrings/githubcli-archive-keyring.gpg \
   && echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | sudo tee /etc/apt/sources.list.d/github-cli.list > /dev/null \
   && sudo apt-get update \
   && sudo apt-get install gh -y
   ```

2. **Authenticate gh:**
   ```bash
   gh auth login
   ```

3. **Clone the repo into WSL filesystem:**
   ```bash
   mkdir -p ~/dev
   cd ~/dev
   gh repo clone youruser/voronoi-reactive-shader
   ```

4. **Git submodules:**
   ```bash
   cd voronoi-reactive-shader

   # If the submodule uses SSH and you only have HTTPS auth:
   git config submodule.vendor/ffgl-rs.url https://github.com/wday/ffgl-rs.git

   git submodule update --init --recursive
   ```

5. **Node.js 22+:**
   ```bash
   curl -fsSL https://deb.nodesource.com/setup_22.x | sudo -E bash -
   sudo apt-get install -y nodejs
   ```

6. **Preview dependencies:**
   ```bash
   cd preview && npm install
   ```

## Building the FFGL Plugin

The build uses the **Windows** Rust toolchain (not a WSL-native one) since the output is a Windows
`.dll` that Resolume loads. The build is invoked directly from WSL — no need to switch to a Windows
shell.

### Build command (from WSL)

```bash
cd /mnt/c && cmd.exe /c "pushd \\\\wsl\$\\Ubuntu\\home\\alien\\dev\\voronoi-reactive-shader\\vendor\\ffgl-rs && set PATH=C:\Users\alien\scoop\apps\rustup\current\.cargo\bin;C:\Users\alien\scoop\shims;%PATH% && set ISF_SOURCE=Z:\home\alien\dev\voronoi-reactive-shader\shaders\voronoi_reactive.fs && set ISF_NAME=voronoi_reactive && set CARGO_TARGET_DIR=C:\Users\alien\.cargo-target\ffgl-rs && cargo build --release -p ffgl-isf"
```

### Key details

- **`pushd \\wsl$\Ubuntu\...`** maps the WSL filesystem to a drive letter (typically `Z:`), avoiding
  CMD's lack of UNC path support as a current directory.
- **`CARGO_TARGET_DIR`** must be on the Windows filesystem (`C:\`). Building with the target dir on
  the WSL filesystem (`Z:\`) causes a cargo dep-info parsing bug with backslash-terminated UNC paths.
- **`ISF_SOURCE`** — absolute Windows path to the ISF shader file. Uses the `Z:` mapping from `pushd`.
- **`ISF_NAME`** — plugin display name in Resolume (max 16 chars). Omitting this causes the plugin to
  show a default name.
- **`PATH`** must include the scoop rustup cargo bin and scoop shims (for `clang.dll`).

### Build output

```
C:\Users\alien\.cargo-target\ffgl-rs\release\ffgl_isf.dll
```

## Deploying to Resolume

Copy the DLL to Resolume Avenue's Extra Effects directory:

```bash
cp /mnt/c/Users/alien/.cargo-target/ffgl-rs/release/ffgl_isf.dll \
   "/mnt/c/Users/alien/Documents/Resolume Avenue/Extra Effects/ffgl_isf.dll"
```

Restart Resolume Avenue. The plugin appears in the Sources panel as `*voronoi_reactive`.

### Resolume plugin directory

Resolume Avenue scans `C:\Users\<user>\Documents\Resolume Avenue\Extra Effects\` on startup. Create
this directory if it doesn't exist. Verify loading in the log:

```
%LOCALAPPDATA%\Resolume Avenue\Resolume Avenue log.txt
```

Look for:
```
ra::WinPluginInstance::load: Loading plugin '...\ffgl_isf.dll'
Plugin was successfully loaded.
```

## Browser Preview (shader iteration)

For fast shader iteration without rebuilding the DLL:

```bash
./scripts/preview.sh
# → http://localhost:9000
```

This runs a Node.js server with hot-reload — edit `shaders/voronoi_reactive.fs`, save, and the
browser updates instantly with all parameter sliders.

## Patches Applied to ffgl-rs

The upstream ffgl-rs needed one change for Windows 64-bit builds:

**`ffgl-core/build.rs`** — Added `.layout_tests(false)` to both `bindgen::Builder` calls. Without
this, bindgen generates struct size assertions for transitively-included Windows SDK types (`_GUID`,
`_CONTEXT`, `_DEBUG_EVENT`, etc.) that fail on x86_64 due to 32-bit vs 64-bit size differences. The
FFGL bindings themselves are unaffected.

## Terminal Setup (WezTerm)

WezTerm is the recommended terminal. Install via scoop:

```powershell
scoop bucket add extras
scoop install extras/wezterm
```

Config lives at `C:\Users\alien\.config\wezterm\wezterm.lua`. A useful launch menu covers all the
shells you'll need, including a VS Developer Shell for any Windows-native debugging:

```lua
local wezterm = require 'wezterm'

return {
  default_prog = { 'wsl.exe', '--distribution', 'Ubuntu', '--exec', '/bin/zsh' },
  launch_menu = {
    { label = 'WSL Ubuntu', args = { 'wsl.exe', '--distribution', 'Ubuntu', '--exec', '/bin/zsh' } },
    { label = 'PowerShell', args = { 'powershell.exe', '-NoLogo' } },
    { label = 'VS Developer Shell', args = {
      'cmd.exe', '/k',
      'C:\\Program Files (x86)\\Microsoft Visual Studio\\2022\\BuildTools\\Common7\\Tools\\VsDevCmd.bat'
    }},
  },
  font = wezterm.font('Cascadia Code', { weight = 'Regular' }),
  font_size = 13.0,
  color_scheme = 'Tokyo Night',
  hide_tab_bar_if_only_one_tab = true,
  window_padding = { left = 12, right = 12, top = 8, bottom = 8 },
  enable_scroll_bar = false,
  audible_bell = 'Disabled',
}
```

Open the launcher with `Ctrl+Shift+Space`.

## Troubleshooting

**`libclang.dll` not found** — Install LLVM: `scoop install llvm`. Ensure scoop shims are on PATH.

**`malformed dep-info format, trailing \`** — The cargo target directory is on the WSL filesystem.
Set `CARGO_TARGET_DIR` to a `C:\` path.

**Plugin not appearing in Resolume** — Check that the `Extra Effects` directory exists and the log
shows the DLL loading. Restart Resolume after deploying.

**Plugin shows wrong name** — Rebuild with `ISF_NAME=voronoi_reactive` set.

**`cargo` not recognized** — Ensure PATH includes `C:\Users\alien\scoop\apps\rustup\current\.cargo\bin`.
If rustup was just installed, open a fresh shell first.

**`rustup` not found after `scoop install rustup`** — Open a fresh shell. Scoop shims don't take
effect in the current session.