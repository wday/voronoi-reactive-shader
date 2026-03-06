# Build and deploy FFGL plugin from WSL via Windows cargo
# Usage:
#   make build       - compile the FFGL DLL
#   make deploy      - copy DLL to Resolume Extra Effects
#   make release     - build + deploy
#   make preview     - start browser preview server

SHADER        := shaders/voronoi_reactive.fs
ISF_NAME      := voronoi_reactive
WSL_DISTRO    := Ubuntu
WSL_USER      := alien

# Windows paths (no trailing spaces!)
WIN_CARGO_BIN := C:\Users\$(WSL_USER)\scoop\apps\rustup\current\.cargo\bin
WIN_SCOOP     := C:\Users\$(WSL_USER)\scoop\shims
WIN_TARGET    := C:\Users\$(WSL_USER)\.cargo-target\ffgl-rs
WIN_ISF_SRC   := Z:\home\$(WSL_USER)\dev\voronoi-reactive-shader\$(subst /,\,$(SHADER))
WIN_FFGL_DIR  := Z:\home\$(WSL_USER)\dev\voronoi-reactive-shader\ffgl-rs

# Linux paths
DLL_OUTPUT    := /mnt/c/Users/$(WSL_USER)/.cargo-target/ffgl-rs/release/ffgl_isf.dll
RESOLUME_DIR  := /mnt/c/Users/$(WSL_USER)/Documents/Resolume Avenue/Extra Effects

.PHONY: build deploy release preview clean

build:
	cd /mnt/c && cmd.exe /c \
		"pushd \\\\wsl$$\$(WSL_DISTRO)\home\$(WSL_USER)\dev\voronoi-reactive-shader\ffgl-rs&&set PATH=$(WIN_CARGO_BIN);$(WIN_SCOOP);%PATH%&&set ISF_SOURCE=$(WIN_ISF_SRC)&&set ISF_NAME=$(ISF_NAME)&&set CARGO_TARGET_DIR=$(WIN_TARGET)&&cargo build --release -p ffgl-isf"

deploy: $(DLL_OUTPUT)
	@mkdir -p "$(RESOLUME_DIR)"
	cp "$(DLL_OUTPUT)" "$(RESOLUME_DIR)/ffgl_isf.dll"
	@echo "Deployed to $(RESOLUME_DIR)/ffgl_isf.dll"
	@echo "Restart Resolume to load the updated plugin."

release: build deploy

preview:
	./scripts/preview.sh

clean:
	rm -f "$(DLL_OUTPUT)"
