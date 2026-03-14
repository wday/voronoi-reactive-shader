# vendor/

Third-party dependencies managed as git submodules.

| Directory | Upstream | Pin | Purpose |
|-----------|----------|-----|---------|
| `ffgl-rs/` | [wday/ffgl-rs](https://github.com/wday/ffgl-rs) | branch `fix/windows-x64-bindgen-layout-tests` | FFGL Rust bindings + ISF build pipeline |
| `spout2/` | [leadedge/Spout2](https://github.com/leadedge/Spout2) | tag `2.007.017` | Spout2 SDK for GPU texture sharing (Windows) |

## Updating submodules

```bash
# Initialize after clone
git submodule update --init --recursive

# Update a submodule to a new commit
cd vendor/ffgl-rs
git fetch origin
git checkout <new-commit>
cd ../..
git add vendor/ffgl-rs
git commit -m "vendor: bump ffgl-rs to <commit>"
```
