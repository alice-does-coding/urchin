.PHONY: help deps compile test test.watch iex run seed dev stop open play clean fmt

PORT ?= 4000
URL  := http://127.0.0.1:$(PORT)

help: ## list available targets
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_.-]+:.*?## / {printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

deps: ## fetch and compile dependencies
	mix deps.get
	mix deps.compile

compile: ## compile the app
	mix compile

test: ## run the full test suite
	mix test

test.watch: ## re-run tests on file change (needs fswatch)
	@command -v fswatch >/dev/null || { echo "install fswatch: brew install fswatch"; exit 1; }
	@fswatch -o lib test | xargs -n1 -I{} mix test --color

iex: ## interactive shell with the supervision tree booted
	iex -S mix

run: ## boot the dashboard, no seed (Ctrl-C to quit)
	elixir --no-halt -S mix run

seed: stop ## boot the dashboard with alice/bob (awake) and clarice (asleep)
	@echo "==> seeding minds, dashboard at $(URL)"
	@elixir --no-halt -S mix run -e '\
		{:ok, _} = Urchin.spawn_mind("alice"); \
		{:ok, _} = Urchin.spawn_mind("bob"); \
		{:ok, _} = Urchin.spawn_mind("clarice"); \
		Urchin.Mind.sleep("clarice")' &
	@echo "==> backgrounded — \`make stop\` to kill, \`make open\` to view"

dev: stop ## full dev: stub LLM if no API key, seed minds, run demo, open browser. Ctrl-C to quit.
	@echo "==> dev mode at $(URL) — opening browser in 3s, Ctrl-C to quit"
	@(sleep 3 && open $(URL) 2>/dev/null || xdg-open $(URL) 2>/dev/null) &
	@elixir --no-halt -S mix run dev/seed.exs

stop: ## kill any process listening on $$PORT
	@pid=$$(lsof -t -iTCP:$(PORT) -sTCP:LISTEN 2>/dev/null); \
	if [ -n "$$pid" ]; then echo "==> killing pid $$pid on :$(PORT)"; kill $$pid; sleep 1; fi

open: ## open the dashboard in the default browser
	@open $(URL) || xdg-open $(URL)

play: seed ## seed + open in browser
	@sleep 2
	@$(MAKE) open

clean: ## remove build artifacts
	mix clean
	rm -rf _build deps/.mix

fmt: ## format Elixir source
	mix format
