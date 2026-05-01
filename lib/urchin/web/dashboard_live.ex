defmodule Urchin.Web.DashboardLive do
  use Phoenix.LiveView

  alias Urchin.{Budget, Mind}
  alias Urchin.Web.TelemetryRelay

  @feed_limit 50
  @refresh_ms 2_000

  @counter_keys ~w(
    throttle_acquire throttle_reject budget_exceeded
    mind_tick mind_thought llm_request llm_response
  )a

  @impl true
  def mount(_params, _session, socket) do
    if connected?(socket) do
      Phoenix.PubSub.subscribe(Urchin.PubSub, "world")
      Phoenix.PubSub.subscribe(Urchin.PubSub, TelemetryRelay.topic())
      :timer.send_interval(@refresh_ms, :refresh_minds)
    end

    {:ok,
     socket
     |> assign(:page_title, "dashboard")
     |> assign(:minds, list_minds())
     |> assign(:feed, [])
     |> assign(:counters, blank_counters())
     |> assign(:counter_keys, @counter_keys)}
  end

  @impl true
  def handle_info({:thought, id, content}, socket) do
    entry = %{id: id, content: content, ts: timestamp()}
    feed = [entry | socket.assigns.feed] |> Enum.take(@feed_limit)
    {:noreply, assign(socket, :feed, feed)}
  end

  def handle_info({:telemetry, event, _measurements, _metadata}, socket) do
    {:noreply, update(socket, :counters, &bump(&1, counter_key(event)))}
  end

  def handle_info(:refresh_minds, socket) do
    {:noreply, assign(socket, :minds, list_minds())}
  end

  def handle_info(_, socket), do: {:noreply, socket}

  @impl true
  def render(assigns) do
    ~H"""
    <header>
      <h1>urchin · live agent dashboard</h1>
    </header>
    <main>
      <section class="panel full">
        <h2>minds ({length(@minds)})</h2>
        <%= if @minds == [] do %>
          <div class="empty">no minds spawned yet — Urchin.spawn_mind("alice")</div>
        <% else %>
          <table>
            <thead>
              <tr><th>id</th><th>node</th><th>status</th><th>messages</th><th>turns used</th></tr>
            </thead>
            <tbody>
              <tr :for={m <- @minds}>
                <td>{m.id}</td>
                <td>{m.node}</td>
                <td><span class={"pill #{m.status}"}>{m.status}</span></td>
                <td>{m.message_count}</td>
                <td>{m.turns_used}</td>
              </tr>
            </tbody>
          </table>
        <% end %>
      </section>

      <section class="panel">
        <h2>world feed</h2>
        <div class="feed">
          <%= if @feed == [] do %>
            <div class="empty">no thoughts yet</div>
          <% else %>
            <div :for={t <- @feed} class="thought">
              <span class="ts">{t.ts}</span>
              <span class="id">[{t.id}]</span>
              <span>{t.content}</span>
            </div>
          <% end %>
        </div>
      </section>

      <section class="panel">
        <h2>counters</h2>
        <div class="counters">
          <div :for={k <- @counter_keys} class="counter">
            <div class="n">{Map.get(@counters, k, 0)}</div>
            <div class="l">{format_label(k)}</div>
          </div>
        </div>
      </section>
    </main>
    """
  end

  # public for tests
  def counter_keys, do: @counter_keys

  defp blank_counters, do: Map.new(@counter_keys, &{&1, 0})

  defp bump(counters, nil), do: counters
  defp bump(counters, key), do: Map.update(counters, key, 1, &(&1 + 1))

  defp counter_key([:urchin, :throttle, :acquire]), do: :throttle_acquire
  defp counter_key([:urchin, :throttle, :reject]), do: :throttle_reject
  defp counter_key([:urchin, :budget, :exceeded]), do: :budget_exceeded
  defp counter_key([:urchin, :mind, :tick]), do: :mind_tick
  defp counter_key([:urchin, :mind, :thought]), do: :mind_thought
  defp counter_key([:urchin, :llm, :request]), do: :llm_request
  defp counter_key([:urchin, :llm, :response]), do: :llm_response
  defp counter_key(_), do: nil

  defp format_label(key) do
    key |> Atom.to_string() |> String.replace("_", " ")
  end

  defp list_minds do
    Urchin.Registry
    |> Horde.Registry.select([{{:"$1", :"$2", :_}, [], [{{:"$1", :"$2"}}]}])
    |> Enum.map(fn {id, pid} -> describe_mind(id, pid) end)
    |> Enum.sort_by(& &1.id)
  end

  defp describe_mind(id, pid) do
    %{
      id: id,
      node: node(pid),
      status: safe_status(id),
      message_count: safe_history_length(id),
      turns_used: Budget.used(id)
    }
  end

  defp safe_status(id) do
    Mind.status(id)
  catch
    :exit, _ -> :unknown
  end

  defp safe_history_length(id) do
    length(Mind.history(id))
  catch
    :exit, _ -> 0
  end

  defp timestamp do
    Time.utc_now() |> Time.truncate(:second) |> Time.to_string()
  end
end
