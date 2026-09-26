# Plugin build and deploy system
# Reads plugins.json for the plugin registry.
#
# Usage:
#   make build                          - build all plugins
#   make deploy                         - deploy all plugins to Resolume
#   make release                        - build + deploy all
#   make build PLUGIN=voronoi_reactive  - build one plugin
#   make deploy PLUGIN=voronoi_reactive - deploy one plugin
#   make preview                        - start browser preview server
#   make list                           - list registered plugins

REGISTRY      := plugins.json

ifeq ($(shell uname -s),Darwin)
MACOS         := 1
TARGET_DIR    := build/macos
RESOLUME_DIR  ?= $(HOME)/Documents/Resolume Avenue/Extra Effects
export RESOLUME_DIR
else
WSL_USER      := alien
TARGET_DIR    := /mnt/c/Users/$(WSL_USER)/.cargo-target/ffgl-rs/release
RESOLUME_DIR  := /mnt/c/Users/$(WSL_USER)/Documents/Resolume Avenue/Extra Effects
endif

PLUGINS_ALL   := $(shell python3 -c "import json; [print(p['name']) for p in json.load(open('$(REGISTRY)'))['plugins']]" 2>/dev/null)

.PHONY: build deploy release preview clean list

list:
	@python3 -c "\
	import json; \
	plugins = json.load(open('$(REGISTRY)'))['plugins']; \
	print('Registered plugins:'); \
	[print(f\"  {p['name']:20s} type={p['type']:5s} dll={p['dll']}\") for p in plugins]"

build:
ifdef PLUGIN
	./scripts/build-plugin.sh $(PLUGIN)
else
	@for p in $(PLUGINS_ALL); do ./scripts/build-plugin.sh $$p; done
endif

deploy:
ifdef PLUGIN
	@$(MAKE) --no-print-directory _deploy_one NAME=$(PLUGIN)
else
	@for p in $(PLUGINS_ALL); do $(MAKE) --no-print-directory _deploy_one NAME=$$p; done
endif

release: build deploy

ifdef MACOS
_deploy_one:
	@./scripts/deploy-mac.sh $(NAME)
else
_deploy_one:
	$(eval PDLL := $(shell python3 -c "import json; p=[p for p in json.load(open('$(REGISTRY)'))['plugins'] if p['name']=='$(NAME)'][0]; print(p['dll'])"))
	@mkdir -p "$(RESOLUME_DIR)"
	@if [ ! -f "$(TARGET_DIR)/$(PDLL)" ]; then \
		echo "==> ERROR: $(TARGET_DIR)/$(PDLL) not found. Run 'make build PLUGIN=$(NAME)' first." >&2; \
		exit 1; \
	elif cp "$(TARGET_DIR)/$(PDLL)" "$(RESOLUME_DIR)/$(PDLL)"; then \
		echo "==> Deployed $(PDLL) to $(RESOLUME_DIR)/"; \
	else \
		echo "==> ERROR: could not copy $(PDLL) — is Resolume holding the DLL? Close it and re-run." >&2; \
		exit 1; \
	fi
endif

preview:
	./scripts/preview.sh

clean:
ifdef MACOS
	rm -rf "$(TARGET_DIR)"
else
	rm -f "$(TARGET_DIR)"/*.dll
endif
