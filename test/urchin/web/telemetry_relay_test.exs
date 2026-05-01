defmodule Urchin.Web.TelemetryRelayTest do
  use ExUnit.Case, async: false

  alias Urchin.Web.TelemetryRelay

  setup do
    Phoenix.PubSub.subscribe(Urchin.PubSub, TelemetryRelay.topic())
    on_exit(fn -> Phoenix.PubSub.unsubscribe(Urchin.PubSub, TelemetryRelay.topic()) end)
    %{}
  end

  test "rebroadcasts a known urchin event to the dashboard topic" do
    :telemetry.execute([:urchin, :mind, :thought], %{}, %{id: "m1"})

    assert_receive {:telemetry, [:urchin, :mind, :thought], %{}, %{id: "m1"}}
  end

  test "carries measurements and metadata through" do
    :telemetry.execute([:urchin, :llm, :response], %{status: 200}, %{model: "test"})

    assert_receive {:telemetry, [:urchin, :llm, :response], %{status: 200},
                    %{model: "test"}}
  end

  test "ignores events outside the urchin namespace" do
    :telemetry.execute([:other, :event], %{}, %{})
    refute_receive {:telemetry, [:other, :event], _, _}, 50
  end
end
