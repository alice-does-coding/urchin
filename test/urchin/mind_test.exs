defmodule Urchin.MindTest do
  use ExUnit.Case, async: false

  alias Urchin.{Budget, Mind}

  setup do
    stub = :"urchin_mind_stub_#{System.unique_integer([:positive])}"
    Application.put_env(:urchin, :req_plug, {Req.Test, stub})

    Req.Test.stub(stub, fn conn ->
      Req.Test.json(conn, %{"content" => [%{"text" => "ack"}]})
    end)

    on_exit(fn -> Application.delete_env(:urchin, :req_plug) end)

    id = "mind-#{System.unique_integer([:positive])}"
    Budget.reset(id)
    {:ok, mind_pid} = Urchin.spawn_mind(id)
    Req.Test.allow(stub, self(), mind_pid)

    on_exit(fn ->
      case Horde.Registry.lookup(Urchin.Registry, id) do
        [{pid, _}] -> Horde.DynamicSupervisor.terminate_child(Urchin.MindSupervisor, pid)
        _ -> :ok
      end
    end)

    %{id: id, stub: stub}
  end

  test "spawned mind starts awake with no history", %{id: id} do
    assert Mind.status(id) == :awake
    assert Mind.history(id) == []
  end

  test "user message round-trips through stubbed LLM", %{id: id} do
    assert {:ok, "ack"} = Mind.send_message(id, :user, "hi")

    assert [
             %{role: :user, content: "hi"},
             %{role: :assistant, content: "ack"}
           ] = Mind.history(id)
  end

  test "non-user message is appended without calling the LLM", %{id: id} do
    assert {:ok, %{role: :system, content: "you are a fish"}} =
             Mind.send_message(id, :system, "you are a fish")

    assert [%{role: :system, content: "you are a fish"}] = Mind.history(id)
  end

  test "sleep blocks new messages, wake unblocks", %{id: id} do
    Mind.sleep(id)
    # cast — wait for it to land
    assert Mind.status(id) == :asleep
    assert {:error, :asleep} = Mind.send_message(id, :user, "hi")

    Mind.wake(id)
    assert Mind.status(id) == :awake
    assert {:ok, "ack"} = Mind.send_message(id, :user, "hi")
  end

  test "user message broadcasts a :thought to the world topic", %{id: id} do
    Phoenix.PubSub.subscribe(Urchin.PubSub, "world")

    {:ok, _} = Mind.send_message(id, :user, "hi")

    assert_receive {:thought, ^id, "ack"}, 500
  end

  test "mind ignores its own thoughts on the world topic", %{id: id} do
    Phoenix.PubSub.broadcast(Urchin.PubSub, "world", {:thought, id, "self-talk"})
    # give it a moment to process
    Process.sleep(50)
    assert Mind.history(id) == []
  end

  test "another mind's thought is appended to history as a user message", %{id: id} do
    Phoenix.PubSub.broadcast(Urchin.PubSub, "world", {:thought, "other", "hello"})
    Process.sleep(50)
    assert [%{role: :user, content: "[other] hello"}] = Mind.history(id)
  end

  test "tick triggers think when a peer thought is dangling", %{id: id} do
    Phoenix.PubSub.subscribe(Urchin.PubSub, "world")

    Phoenix.PubSub.broadcast(Urchin.PubSub, "world", {:thought, "other", "yo"})
    Process.sleep(20)

    [{mind_pid, _}] = Horde.Registry.lookup(Urchin.Registry, id)
    send(mind_pid, :tick)

    assert_receive {:thought, ^id, "ack"}, 500

    history = Mind.history(id)
    assert List.last(history) == %{role: :assistant, content: "ack"}
  end

  test "tick stays quiet when nothing is dangling", %{id: id} do
    Phoenix.PubSub.subscribe(Urchin.PubSub, "world")

    [{mind_pid, _}] = Horde.Registry.lookup(Urchin.Registry, id)
    send(mind_pid, :tick)

    refute_receive {:thought, ^id, _}, 100
    assert Mind.history(id) == []
  end

  test "tick stays quiet when last message is from self", %{id: id} do
    Phoenix.PubSub.subscribe(Urchin.PubSub, "world")

    {:ok, _} = Mind.send_message(id, :user, "hi")
    assert_receive {:thought, ^id, "ack"}, 500

    [{mind_pid, _}] = Horde.Registry.lookup(Urchin.Registry, id)
    send(mind_pid, :tick)

    refute_receive {:thought, ^id, _}, 100
  end

  test "send_message returns :budget_exceeded when budget is drained", %{id: id} do
    Application.put_env(:urchin, :budget_turns, 1)
    Budget.reset(id)

    assert {:ok, "ack"} = Mind.send_message(id, :user, "first")
    assert {:error, :budget_exceeded} = Mind.send_message(id, :user, "second")
  after
    Application.delete_env(:urchin, :budget_turns)
  end
end
