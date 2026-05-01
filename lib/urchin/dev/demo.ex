defmodule Urchin.Dev.Demo do
  @moduledoc """
  Animates the system so the dashboard shows actual motion in dev. On each
  tick it picks one of three actions: broadcast a narrator thought (which
  wakes minds via PubSub), toggle a random mind's status, or emit a
  telemetry event so the counters move.

  Started by `dev/seed.exs`, never auto-started in the supervision tree.
  """
  use GenServer

  @default_interval_ms 1_500

  @phrases [
    "what if minds were just stories telling themselves",
    "i found a stone in my thoughts",
    "the bus is louder than i remembered",
    "do other minds dream?",
    "alice? are you there?",
    "🐚 🌊 🐡",
    "everyone breathes",
    "the silence is loud here",
    "...",
    "i was just about to say that"
  ]

  def start_link(opts \\ []) do
    start_opts = Keyword.take(opts, [:name])
    GenServer.start_link(__MODULE__, opts, start_opts)
  end

  @impl true
  def init(opts) do
    interval = Keyword.get(opts, :interval_ms, @default_interval_ms)
    Process.send_after(self(), :tick, interval)
    {:ok, %{interval: interval, n: 0}}
  end

  @impl true
  def handle_info(:tick, state) do
    act(rem(state.n, 3))
    Process.send_after(self(), :tick, state.interval)
    {:noreply, %{state | n: state.n + 1}}
  end

  defp act(0), do: broadcast_narrator()
  defp act(1), do: toggle_random_mind()
  defp act(2), do: emit_random_telemetry()

  defp broadcast_narrator do
    Phoenix.PubSub.broadcast(
      Urchin.PubSub,
      "world",
      {:thought, "narrator", Enum.random(@phrases)}
    )
  end

  defp toggle_random_mind do
    case mind_ids() do
      [] ->
        :noop

      ids ->
        id = Enum.random(ids)

        case safe_status(id) do
          :awake -> Urchin.Mind.sleep(id)
          :asleep -> Urchin.Mind.wake(id)
          _ -> :noop
        end
    end
  end

  defp emit_random_telemetry do
    case Enum.random([:throttle_acquire, :throttle_reject, :budget_exceeded, :mind_tick]) do
      :throttle_acquire ->
        :telemetry.execute([:urchin, :throttle, :acquire], %{tokens: 5}, %{})

      :throttle_reject ->
        :telemetry.execute([:urchin, :throttle, :reject], %{}, %{})

      :budget_exceeded ->
        :telemetry.execute([:urchin, :budget, :exceeded], %{used: 100}, %{id: "demo"})

      :mind_tick ->
        :telemetry.execute([:urchin, :mind, :tick], %{}, %{id: "demo"})
    end
  end

  defp mind_ids do
    Horde.Registry.select(Urchin.Registry, [{{:"$1", :"$2", :_}, [], [:"$1"]}])
  end

  defp safe_status(id) do
    Urchin.Mind.status(id)
  catch
    :exit, _ -> :unknown
  end

  @doc false
  def phrases, do: @phrases
end
