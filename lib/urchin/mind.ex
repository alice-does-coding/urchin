defmodule Urchin.Mind do
  use GenServer

  defstruct [:id, messages: [], status: :awake]

  # Client API

  def start_link(id) do
    GenServer.start_link(__MODULE__, id, name: via(id))
  end

  def send_message(id, role, content) do
    GenServer.call(via(id), {:message, role, content})
  end

  def history(id) do
    GenServer.call(via(id), :history)
  end

  def status(id) do
    GenServer.call(via(id), :status)
  end

  def sleep(id) do
    GenServer.cast(via(id), :sleep)
  end

  def wake(id) do
    GenServer.cast(via(id), :wake)
  end

  # Server callbacks

  @impl true
  def init(id) do
    {:ok, %__MODULE__{id: id}}
  end

  @impl true
  def handle_call({:message, _role, _content}, _from, %{status: :asleep} = state) do
    {:reply, {:error, :asleep}, state}
  end

  def handle_call({:message, :user, content}, _from, state) do
    user_msg = %{role: :user, content: content}
    messages = state.messages ++ [user_msg]

    case Urchin.LLM.complete(messages) do
      {:ok, reply} ->
        assistant_msg = %{role: :assistant, content: reply}
        {:reply, {:ok, reply}, %{state | messages: messages ++ [assistant_msg]}}

      {:error, _} = err ->
        {:reply, err, %{state | messages: messages}}
    end
  end

  def handle_call({:message, role, content}, _from, state) do
    msg = %{role: role, content: content}
    {:reply, {:ok, msg}, %{state | messages: state.messages ++ [msg]}}
  end

  def handle_call(:history, _from, state) do
    {:reply, state.messages, state}
  end

  def handle_call(:status, _from, state) do
    {:reply, state.status, state}
  end

  @impl true
  def handle_cast(:sleep, state) do
    {:noreply, %{state | status: :asleep}}
  end

  def handle_cast(:wake, state) do
    {:noreply, %{state | status: :awake}}
  end

  defp via(id) do
    {:via, Registry, {Urchin.Registry, id}}
  end
end
