defmodule Urchin.Web.Endpoint do
  use Phoenix.Endpoint, otp_app: :urchin

  @session_options [
    store: :cookie,
    key: "_urchin_key",
    signing_salt: "urchin-sess",
    same_site: "Lax"
  ]

  socket "/live", Phoenix.LiveView.Socket,
    websocket: [connect_info: [session: @session_options]]

  plug Plug.Static, at: "/assets/phoenix", from: :phoenix
  plug Plug.Static, at: "/assets/phoenix_html", from: :phoenix_html
  plug Plug.Static, at: "/assets/phoenix_live_view", from: :phoenix_live_view

  plug Urchin.Web.StaticAssets

  plug Plug.RequestId
  plug Plug.Telemetry, event_prefix: [:urchin, :endpoint]

  plug Plug.Parsers,
    parsers: [:urlencoded, :multipart, :json],
    pass: ["*/*"],
    json_decoder: Phoenix.json_library()

  plug Plug.MethodOverride
  plug Plug.Head
  plug Plug.Session, @session_options
  plug Urchin.Web.Router
end
