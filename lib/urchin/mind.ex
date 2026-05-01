defmodule Urchin.Mind do
  use GenServer

  alias Urchin.{Budget, LLM}

  @tick_min 5_000
  @tick_jitter 5_000

  defstruct [:id, messages: [], status: :awake, topics: []]

  # Client API

  def child_spec(id) do
    %{id: {__MODULE__, id}, start: {__MODULE__, :start_link, [id]}, restart: :transient}
  end

  def start_link(id) do
    GenServer.start_link(__MODULE__, id, name: via(id))
  end

  def send_message(id, role, content) do
    GenServer.call(via(id), {:message, role, content})
  end

  def history(id), do: GenServer.call(via(id), :history)
  def status(id), do: GenServer.call(via(id), :status)
  def sleep(id), do: GenServer.cast(via(id), :sleep)
  def wake(id), do: GenServer.cast(via(id), :wake)

  # Server callbacks

  @impl true
  def init(id) do
    topics = ["world", "mind:#{id}"]
    Enum.each(topics, &Phoenix.PubSub.subscribe(Urchin.PubSub, &1))
    schedule_tick()
    {:ok, %__MODULE__{id: id, topics: topics}}
  end

  @impl true
  def handle_call({:message, _role, _content}, _from, %{status: :asleep} = state) do
    {:reply, {:error, :asleep}, state}
  end

  def handle_call({:message, :user, content}, _from, state) do
    user_msg = %{role: :user, content: content}
    messages = state.messages ++ [user_msg]

    case think(state.id, messages) do
      {:ok, reply} ->
        assistant_msg = %{role: :assistant, content: reply}
        broadcast_thought(state.id, reply)
        {:reply, {:ok, reply}, %{state | messages: messages ++ [assistant_msg]}}

      {:error, _} = err ->
        {:reply, err, %{state | messages: messages}}
    end
  end

  def handle_call({:message, role, content}, _from, state) do
    msg = %{role: role, content: content}
    {:reply, {:ok, msg}, %{state | messages: state.messages ++ [msg]}}
  end

  def handle_call(:history, _from, state), do: {:reply, state.messages, state}
  def handle_call(:status, _from, state), do: {:reply, state.status, state}

  @impl true
  def handle_cast(:sleep, state), do: {:noreply, %{state | status: :asleep}}
  def handle_cast(:wake, state), do: {:noreply, %{state | status: :awake}}

  @impl true
  def handle_info(:tick, %{status: :asleep} = state) do
    schedule_tick()
    {:noreply, state}
  end

  def handle_info(:tick, state) do
    :telemetry.execute([:urchin, :mind, :tick], %{}, %{id: state.id})
    schedule_tick()

    if should_think?(state) do
      case think(state.id, state.messages) do
        {:ok, reply} ->
          assistant_msg = %{role: :assistant, content: reply}
          broadcast_thought(state.id, reply)
          {:noreply, %{state | messages: state.messages ++ [assistant_msg]}}

        {:error, _} ->
          {:noreply, state}
      end
    else
      {:noreply, state}
    end
  end

  def handle_info({:thought, from_id, _content}, %{id: id} = state) when from_id == id do
    {:noreply, state}
  end

  def handle_info({:thought, from_id, content}, state) do
    msg = %{role: :user, content: "[#{from_id}] #{content}"}
    {:noreply, %{state | messages: state.messages ++ [msg]}}
  end

  def handle_info(_, state), do: {:noreply, state}

  # Helpers

  defp think(id, messages) do
    with :ok <- Budget.charge(id),
         {:ok, reply} <- LLM.complete(messages) do
      {:ok, reply}
    end
  end

  defp broadcast_thought(id, content) do
    :telemetry.execute([:urchin, :mind, :thought], %{}, %{id: id})
    Phoenix.PubSub.broadcast(Urchin.PubSub, "world", {:thought, id, content})
  end

  defp schedule_tick do
    Process.send_after(self(), :tick, @tick_min + :rand.uniform(@tick_jitter))
  end

  defp should_think?(%{messages: []}), do: false
  defp should_think?(%{messages: messages}), do: List.last(messages).role == :user

  defp via(id), do: {:via, Horde.Registry, {Urchin.Registry, id}}
end
