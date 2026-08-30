# FLOODLINE — shortcuts.
#
#   make              run the game
#   make help         list every command
#
# Lines beginning with `## name: text` below are what `make help` prints, so
# a target and its documentation cannot drift apart.

CARGO ?= cargo
GUI   := gui

ROOT  := $(patsubst %/,%,$(dir $(abspath $(lastword $(MAKEFILE_LIST)))))

.DEFAULT_GOAL := run
.PHONY: run play test check build web serve publish browser-test clean help

## run: play the native build
run play:
	@$(CARGO) run -p $(GUI)

## test: run the whole suite (no window needed)
test:
	@$(CARGO) test --workspace

## browser-test: check the browser build in a real browser (needs the network)
#
# Deliberately not part of `make test`, which stays hermetic and quick. This
# one downloads Chromium the first time and talks to public signalling relays
# every time. packaging/browser/README.md says what each script answers.
browser-test: web
	@$(ROOT)/packaging/browser/run.sh

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
	@echo "  Two players: one hosts and shares the room code or a pasted"
	@echo "  offer. Nothing of ours runs on a server."
