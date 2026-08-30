# FLOODLINE — shortcuts.
#
#   make              run the game
#   make help         list every command
#
# Lines beginning with `## name: text` below are what `make help` prints, so
# a target and its documentation cannot drift apart.

CARGO ?= cargo
GUI   := gui
BOT   := bot

ROOT  := $(patsubst %/,%,$(dir $(abspath $(lastword $(MAKEFILE_LIST)))))

# The signaling server is deployed, never linked: it is not a workspace member
# and appears in no Cargo.toml (design 9.3). `make signal` runs whatever is on
# PATH and tells you how to get one if there is nothing there.
SIGNAL_VERSION := 0.14.0
SIGNAL_PORT    ?= 3536
ROOM           ?= floodline

.DEFAULT_GOAL := run
.PHONY: run play test check build web serve publish signal bot clean help

## run: play the native build
run play:
	@$(CARGO) run -p $(GUI)

## test: run the whole suite (no window needed)
test:
	@$(CARGO) test --workspace

## check: fast type-check of everything, native and wasm, no binaries
check:
	@$(CARGO) check --workspace --all-targets
	@$(CARGO) check -p $(GUI) --target wasm32-unknown-unknown

## build: compile everything without running it
build:
	@$(CARGO) build --workspace

## web: build the browser version into dist/web/
web:
	@$(ROOT)/packaging/package-web.sh

## serve: build the browser version and open it locally
serve: web
	@echo "Serving http://localhost:8080/ — Ctrl-C to stop"
	@(sleep 1 && open http://localhost:8080/) >/dev/null 2>&1 &
	@cd $(ROOT)/dist/web && python3 -m http.server 8080

## publish: copy the web build into docs/ and push it
#
# The workflow in .github/workflows/pages.yml is the route that should be used;
# this is the manual one, kept for the day the workflow is the broken thing.
publish: web
	@mkdir -p $(ROOT)/docs
	@cp -R $(ROOT)/dist/web/. $(ROOT)/docs/
	@touch $(ROOT)/docs/.nojekyll
	@cd $(ROOT) && git add docs && \
	  (git diff --cached --quiet && echo "docs/ unchanged — nothing to publish" \
	   || (git commit -q -m "Publish web build" && git push -q && \
	       echo "Pushed. Pages redeploys in about a minute."))

## signal: run a local matchbox_server on :3536
signal:
	@command -v matchbox_server >/dev/null || { \
	  echo "matchbox_server is not on PATH."; \
	  echo "  cargo install matchbox_server --version $(SIGNAL_VERSION)"; \
	  exit 1; }
	@echo "Signaling on ws://localhost:$(SIGNAL_PORT)/<room>?next=<players>"
	@matchbox_server 0.0.0.0:$(SIGNAL_PORT)

## bot: join a room as a headless peer (make bot ROOM=x)
bot:
	@$(CARGO) run -p $(BOT) -- --room $(ROOM)

## clean: delete build artifacts and packaged output
clean:
	@$(CARGO) clean
	@rm -rf $(ROOT)/dist

## help: list these commands
help:
	@echo "FLOODLINE"
	@echo
	@grep -E '^## [a-z]' $(MAKEFILE_LIST) | sed 's/^## //' \
	  | awk -F': ' '{ printf "  make %-10s %s\n", $$1, $$2 }'
	@echo
	@echo "  Two players: one runs 'make signal', both open the page with"
	@echo "  the same ?room= code."
