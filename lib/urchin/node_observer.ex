defmodule Urchin.NodeObserver do
  @moduledoc """
  Subscribes to node up/down events and keeps the Horde members list in sync
  so minds reshard automatically when the cluster changes.
  """
  use GenServer
  require Logger

  @members [Urchin.Registry, Urchin.MindSupervisor]

  def start_link(_opts) do
    GenServer.start_link(__MODULE__, nil, name: __MODULE__)
  end

  @impl true
  def init(_) do
    :net_kernel.monitor_nodes(true, node_type: :visible)
    set_members()
    {:ok, nil}
  end

  @impl true
  def handle_info({:nodeup, node, _info}, state) do
    Logger.info("urchin: nodeup #{inspect(node)}")
    set_members()
    {:noreply, state}
  end

  def handle_info({:nodedown, node, _info}, state) do
    Logger.info("urchin: nodedown #{inspect(node)}")
    set_members()
    {:noreply, state}
  end

  defp set_members do
    nodes = [Node.self() | Node.list()]

    for member <- @members do
      Horde.Cluster.set_members(member, Enum.map(nodes, &{member, &1}))
    end
  end
end
