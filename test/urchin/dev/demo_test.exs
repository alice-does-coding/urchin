defmodule Urchin.Dev.DemoTest do
  use ExUnit.Case, async: false

  alias Urchin.Dev.Demo
  alias Urchin.Web.TelemetryRelay

  setup do
    Phoenix.PubSub.subscribe(Urchin.PubSub, "world")
    Phoenix.PubSub.subscribe(Urchin.PubSub, TelemetryRelay.topic())

    on_exit(fn ->
      Phoenix.PubSub.unsubscribe(Urchin.PubSub, "world")
      Phoenix.PubSub.unsubscribe(Urchin.PubSub, TelemetryRelay.topic())
    end)

    %{}
  end

  test "advances n on every tick" do
    {:ok, pid} = Demo.start_link(interval_ms: 20)
    Process.sleep(120)
    assert :sys.get_state(pid).n >= 3
    GenServer.stop(pid)
  end

  test "broadcasts a narrator thought on a 0-mod-3 tick" do
    {:ok, pid} = Demo.start_link(interval_ms: 20)

    assert_receive {:thought, "narrator", phrase}, 1_000
    assert phrase in Demo.phrases()

    GenServer.stop(pid)
  end

  test "emits a telemetry event on a 2-mod-3 tick" do
    {:ok, pid} = Demo.start_link(interval_ms: 20)

    assert_receive {:telemetry, [:urchin | _], _, _}, 1_000

    GenServer.stop(pid)
  end

  test "toggling a random mind no-ops cleanly when registry is empty" do
    {:ok, pid} = Demo.start_link(interval_ms: 20)
    Process.sleep(150)
    assert Process.alive?(pid)
    GenServer.stop(pid)
  end

  test "respects custom interval_ms" do
    {:ok, pid} = Demo.start_link(interval_ms: 5_000)
    Process.sleep(50)
    assert :sys.get_state(pid).n == 0
    GenServer.stop(pid)
  end
end
