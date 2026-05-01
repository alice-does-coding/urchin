defmodule Urchin.Telemetry do
  @moduledoc """
  Attaches log handlers for the events Urchin emits. Swap these for a real
  exporter (Prometheus, OpenTelemetry, LiveDashboard) when wiring observability.
  """
  use Supervisor
  require Logger

  @events [
    [:urchin, :throttle, :acquire],
    [:urchin, :throttle, :reject],
    [:urchin, :budget, :exceeded],
    [:urchin, :mind, :tick],
    [:urchin, :mind, :thought],
    [:urchin, :llm, :request],
    [:urchin, :llm, :response]
  ]

  def start_link(opts), do: Supervisor.start_link(__MODULE__, opts, name: __MODULE__)

  @impl true
  def init(_opts) do
    :telemetry.attach_many(
      "urchin-log",
      @events,
      &__MODULE__.handle_event/4,
      nil
    )

    Supervisor.init([], strategy: :one_for_one)
  end

  def handle_event(event, measurements, metadata, _config) do
    Logger.debug(fn ->
      "telemetry #{Enum.join(event, ".")} #{inspect(measurements)} #{inspect(metadata)}"
    end)
  end
end
