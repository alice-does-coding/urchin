defmodule Urchin.Application do
  @moduledoc false

  use Application

  @impl true
  def start(_type, _args) do
    topologies = Application.get_env(:libcluster, :topologies, [])

    children = [
      {Cluster.Supervisor, [topologies, [name: Urchin.ClusterSupervisor]]},
      {Phoenix.PubSub, name: Urchin.PubSub},
      {Horde.Registry, name: Urchin.Registry, keys: :unique, members: :auto},
      {Horde.DynamicSupervisor,
       name: Urchin.MindSupervisor, strategy: :one_for_one, members: :auto},
      Urchin.LLM.Throttle,
      Urchin.Budget,
      {Finch, name: Urchin.Finch},
      Urchin.NodeObserver,
      Urchin.Telemetry,
      Urchin.Web.TelemetryRelay,
      Urchin.Web.Endpoint
    ]

    opts = [strategy: :rest_for_one, name: Urchin.Supervisor]
    Supervisor.start_link(children, opts)
  end
end
