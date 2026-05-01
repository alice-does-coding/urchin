defmodule Urchin.LLM.Throttle do
  @moduledoc """
  Token-bucket gate for LLM calls. Every `Urchin.LLM.complete/1` must call
  `acquire/1` first; if the bucket is empty the caller is told to back off.
  """
  use GenServer

  @default_rate 5
  @default_burst 10
  @default_refill_ms 1_000

  def start_link(opts \\ []) do
    name = Keyword.get(opts, :name, __MODULE__)
    GenServer.start_link(__MODULE__, opts, name: name)
  end

  def acquire(name \\ __MODULE__, timeout \\ 5_000) do
    GenServer.call(name, :acquire, timeout)
  end

  @impl true
  def init(opts) do
    rate = Keyword.get(opts, :rate, @default_rate)
    burst = Keyword.get(opts, :burst, @default_burst)
    refill_ms = Keyword.get(opts, :refill_ms, @default_refill_ms)
    Process.send_after(self(), :refill, refill_ms)
    {:ok, %{tokens: burst, rate: rate, burst: burst, refill_ms: refill_ms}}
  end

  @impl true
  def handle_call(:acquire, _from, %{tokens: tokens} = state) when tokens > 0 do
    :telemetry.execute([:urchin, :throttle, :acquire], %{tokens: tokens - 1}, %{})
    {:reply, :ok, %{state | tokens: tokens - 1}}
  end

  def handle_call(:acquire, _from, state) do
    :telemetry.execute([:urchin, :throttle, :reject], %{}, %{})
    {:reply, {:error, :rate_limited}, state}
  end

  @impl true
  def handle_info(:refill, %{tokens: t, rate: r, burst: b, refill_ms: ms} = state) do
    Process.send_after(self(), :refill, ms)
    {:noreply, %{state | tokens: min(b, t + r)}}
  end
end
