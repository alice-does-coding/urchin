import Config

config :urchin, Urchin.Web.Endpoint,
  url: [host: "localhost"],
  http: [ip: {127, 0, 0, 1}, port: 4000],
  adapter: Bandit.PhoenixAdapter,
  pubsub_server: Urchin.PubSub,
  secret_key_base:
    "5p9mVt8b6kQ1ZXRyKxJzgHJN8JqLm0Ql7YVcX3F4h8nTKQy6jR2WzCfXp1V0sM4u",
  live_view: [signing_salt: "urchin-dev-salt"],
  render_errors: [formats: [html: Urchin.Web.ErrorHTML], layout: false],
  server: true

config :phoenix, :json_library, Jason

import_config "#{config_env()}.exs"
