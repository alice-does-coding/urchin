defmodule Urchin.Web.Router do
  use Phoenix.Router

  import Phoenix.LiveView.Router

  pipeline :browser do
    plug :accepts, ["html"]
    plug :fetch_session
    plug :fetch_live_flash
    plug :put_root_layout, html: {Urchin.Web.Layouts, :root}
    plug :put_secure_browser_headers
  end

  scope "/", Urchin.Web do
    pipe_through :browser
    live "/", DashboardLive, :index
  end
end
