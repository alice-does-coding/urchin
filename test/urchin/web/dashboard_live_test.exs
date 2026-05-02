defmodule Urchin.Web.DashboardLiveTest do
  use ExUnit.Case, async: false

  import Phoenix.ConnTest
  import Phoenix.LiveViewTest

  alias Urchin.Web.TelemetryRelay

  @endpoint Urchin.Web.Endpoint

  setup do
    conn = Phoenix.ConnTest.build_conn()
    %{conn: conn}
  end

  test "renders the three panels on mount", %{conn: conn} do
    {:ok, _view, html} = live(conn, "/")

    assert html =~ "urchin · live agent dashboard"
    assert html =~ "minds"
    assert html =~ "world feed"
    assert html =~ "counters"
  end

  test "renders a csrf-token meta tag so LiveView can connect cleanly", %{conn: conn} do
    html = conn |> get("/") |> response(200)
    assert html =~ ~r|<meta name="csrf-token" content="[^"]+"|
  end

  test "shows the empty-state hint when no minds are spawned", %{conn: conn} do
    {:ok, _view, html} = live(conn, "/")
    assert html =~ "no minds spawned yet"
  end

  test "appends thoughts to the feed when broadcast on the world topic", %{conn: conn} do
    {:ok, view, _} = live(conn, "/")

    Phoenix.PubSub.broadcast(Urchin.PubSub, "world", {:thought, "alice", "hello world"})

    html = render(view)
    assert html =~ "[alice]"
    assert html =~ "hello world"
    refute html =~ "no thoughts yet"
  end

  test "increments counters on telemetry-relay events", %{conn: conn} do
    {:ok, view, _} = live(conn, "/")

    Phoenix.PubSub.broadcast(
      Urchin.PubSub,
      TelemetryRelay.topic(),
      {:telemetry, [:urchin, :mind, :thought], %{}, %{id: "x"}}
    )

    Phoenix.PubSub.broadcast(
      Urchin.PubSub,
      TelemetryRelay.topic(),
      {:telemetry, [:urchin, :mind, :thought], %{}, %{id: "y"}}
    )

    html = render(view)
    # The mind_thought counter should now read 2
    assert html =~ ~r/<div class="n">\s*2\s*<\/div>\s*<div class="l">\s*mind thought/
  end

  test "displays a spawned mind in the grid", %{conn: conn} do
    id = "dash-mind-#{System.unique_integer([:positive])}"
    Urchin.Budget.reset(id)
    {:ok, _pid} = Urchin.spawn_mind(id)

    on_exit(fn ->
      case Horde.Registry.lookup(Urchin.Registry, id) do
        [{pid, _}] -> Horde.DynamicSupervisor.terminate_child(Urchin.MindSupervisor, pid)
        _ -> :ok
      end
    end)

    {:ok, view, _} = live(conn, "/")
    send(view.pid, :refresh_minds)

    html = render(view)
    assert html =~ id
    assert html =~ "awake"
  end
end
