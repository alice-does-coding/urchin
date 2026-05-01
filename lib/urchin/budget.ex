defmodule Urchin.Budget do
  @moduledoc """
  Per-mind quota tracker. ETS-backed so it survives mind crashes/restarts —
  a runaway mind that gets respawned still hits its prior spend.
  """
  use GenServer

  @table :urchin_budget
  @default_turns 100

  def start_link(opts \\ []) do
    GenServer.start_link(__MODULE__, opts, name: __MODULE__)
  end

  def charge(id, cost \\ 1) do
    used = :ets.update_counter(@table, id, {2, cost}, {id, 0, default_limit()})
    [{^id, _, limit}] = :ets.lookup(@table, id)

    if used > limit do
      :telemetry.execute([:urchin, :budget, :exceeded], %{used: used}, %{id: id})
      {:error, :budget_exceeded}
    else
      :ok
    end
  end

  def used(id) do
    case :ets.lookup(@table, id) do
      [{^id, used, _limit}] -> used
      [] -> 0
    end
  end

  def reset(id) do
    :ets.insert(@table, {id, 0, default_limit()})
    :ok
  end

  defp default_limit, do: Application.get_env(:urchin, :budget_turns, @default_turns)

  @impl true
  def init(_opts) do
    :ets.new(@table, [:named_table, :public, :set, write_concurrency: true])
    {:ok, %{}}
  end
end
