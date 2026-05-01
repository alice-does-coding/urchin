defmodule Urchin.Web.TelemetryRelay do
  @moduledoc """
  Attaches to urchin.* telemetry events and rebroadcasts each one to the
  "dashboard" PubSub topic so LiveViews can subscribe and update.
  """
  use GenServer

  @events [
    [:urchin, :throttle, :acquire],
    [:urchin, :throttle, :reject],
    [:urchin, :budget, :exceeded],
    [:urchin, :mind, :tick],
    [:urchin, :mind, :thought],
    [:urchin, :llm, :request],
    [:urchin, :llm, :response]
  ]

  @topic "dashboard"

  def topic, do: @topic
  def events, do: @events

  def start_link(opts \\ []) do
    GenServer.start_link(__MODULE__, opts, name: __MODULE__)
  end

  @impl true
  def init(opts) do
    pubsub = Keyword.get(opts, :pubsub, Urchin.PubSub)
    handler_id = Keyword.get(opts, :handler_id, "urchin-dashboard-relay")

    :telemetry.attach_many(
      handler_id,
      @events,
      &__MODULE__.handle_event/4,
      %{pubsub: pubsub}
    )

    {:ok, %{handler_id: handler_id}}
  end

  @impl true
  def terminate(_reason, %{handler_id: handler_id}) do
    :telemetry.detach(handler_id)
    :ok
  end

  def handle_event(event, measurements, metadata, %{pubsub: pubsub}) do
    Phoenix.PubSub.broadcast(pubsub, @topic, {:telemetry, event, measurements, metadata})
  end
end
