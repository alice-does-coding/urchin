defmodule Urchin.MixProject do
  use Mix.Project

  def project do
    [
      app: :urchin,
      version: "0.1.0",
      elixir: "~> 1.19",
      start_permanent: Mix.env() == :prod,
      deps: deps()
    ]
  end

  def application do
    [
      extra_applications: [:logger],
      mod: {Urchin.Application, []}
    ]
  end

  defp deps do
    [
      {:req, "~> 0.5"},
      {:jason, "~> 1.4"},
      {:finch, "~> 0.18"},
      {:telemetry, "~> 1.2"},
      {:phoenix_pubsub, "~> 2.1"},
      {:horde, "~> 0.9"},
      {:libcluster, "~> 3.4"},
      {:phoenix, "~> 1.7"},
      {:phoenix_live_view, "~> 1.0"},
      {:phoenix_html, "~> 4.1"},
      {:phoenix_template, "~> 1.0"},
      {:bandit, "~> 1.5"},
      {:plug, "~> 1.16"},
      {:lazy_html, ">= 0.1.0", only: :test}
    ]
  end
end
