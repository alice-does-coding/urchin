defmodule Urchin.Web.StaticAssets do
  @moduledoc """
  Plug that serves the inline CSS + JS for the dashboard.
  Avoids needing an esbuild/tailwind pipeline for a dev-only UI.
  """
  @behaviour Plug

  @css """
  * { box-sizing: border-box; }
  body {
    font: 14px/1.5 ui-monospace, "SF Mono", Menlo, monospace;
    margin: 0;
    background: #0e0e10;
    color: #e0e0e0;
  }
  header { padding: 12px 20px; border-bottom: 1px solid #2a2a2e; background: #15151a; }
  h1 { margin: 0; font-size: 16px; font-weight: 600; }
  h2 { margin: 0 0 8px; font-size: 12px; text-transform: uppercase; letter-spacing: 0.08em; color: #888; }
  main { display: grid; grid-template-columns: 2fr 1fr; gap: 16px; padding: 16px 20px; align-items: start; }
  .panel { background: #15151a; border: 1px solid #2a2a2e; border-radius: 6px; padding: 14px; }
  .full { grid-column: 1 / -1; }
  table { width: 100%; border-collapse: collapse; }
  th, td { text-align: left; padding: 6px 8px; border-bottom: 1px solid #2a2a2e; }
  th { color: #888; font-weight: 500; font-size: 12px; }
  .pill { display: inline-block; padding: 1px 8px; border-radius: 9px; font-size: 11px; }
  .awake { background: #143d2c; color: #6ee7b7; }
  .asleep { background: #3d2c14; color: #fcd34d; }
  .feed { max-height: 400px; overflow-y: auto; }
  .thought { padding: 6px 0; border-bottom: 1px solid #2a2a2e; }
  .thought .id { color: #82c8e5; }
  .thought .ts { color: #555; font-size: 11px; }
  .counters { display: grid; grid-template-columns: repeat(2, 1fr); gap: 8px; }
  .counter { background: #1a1a20; padding: 8px 10px; border-radius: 4px; }
  .counter .n { font-size: 20px; font-weight: 600; color: #fff; }
  .counter .l { font-size: 11px; color: #888; text-transform: uppercase; }
  .empty { color: #666; font-style: italic; }
  """

  @js """
  window.addEventListener("DOMContentLoaded", function () {
    var liveSocket = new window.LiveView.LiveSocket(
      "/live",
      window.Phoenix.Socket
    );
    liveSocket.connect();
    window.liveSocket = liveSocket;
  });
  """

  @impl true
  def init(opts), do: opts

  @impl true
  def call(%Plug.Conn{request_path: "/dashboard.css"} = conn, _opts) do
    conn
    |> Plug.Conn.put_resp_content_type("text/css")
    |> Plug.Conn.send_resp(200, @css)
    |> Plug.Conn.halt()
  end

  def call(%Plug.Conn{request_path: "/dashboard.js"} = conn, _opts) do
    conn
    |> Plug.Conn.put_resp_content_type("application/javascript")
    |> Plug.Conn.send_resp(200, @js)
    |> Plug.Conn.halt()
  end

  def call(conn, _opts), do: conn
end
