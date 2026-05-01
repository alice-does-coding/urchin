# Boots the dashboard with a populated, animated system. Run via `make dev`.

alias Urchin.Dev.{Demo, LLMStub}

if System.get_env("ANTHROPIC_API_KEY") in [nil, ""] do
  Application.put_env(:urchin, :req_plug, LLMStub)
  IO.puts("==> no ANTHROPIC_API_KEY — using #{inspect(LLMStub)} for canned responses")
else
  IO.puts("==> ANTHROPIC_API_KEY found — using real Claude API")
end

{:ok, _} = Urchin.spawn_mind("alice")
{:ok, _} = Urchin.spawn_mind("bob")
{:ok, _} = Urchin.spawn_mind("clarice")
{:ok, _} = Demo.start_link([])

IO.puts("==> minds: alice, bob, clarice")
IO.puts("==> demo orchestrator: ticking every 1.5s")
IO.puts("==> dashboard: http://127.0.0.1:4000")
IO.puts("==> Ctrl-C twice to quit")
