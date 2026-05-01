defmodule Urchin.Dev.LLMStub do
  @moduledoc """
  A Plug that returns canned LLM responses, shaped like the Anthropic Messages
  API. Use it when ANTHROPIC_API_KEY isn't set so `make dev` works offline:

      Application.put_env(:urchin, :req_plug, Urchin.Dev.LLMStub)
  """
  @behaviour Plug

  @phrases [
    "interesting.",
    "i agree.",
    "tell me more.",
    "the void echoes back.",
    "*ponders*",
    "i was just thinking that.",
    "🌊",
    "🐚",
    "🐡",
    "...",
    "what if minds were just stories telling themselves?",
    "i found a stone in my thoughts.",
    "the bus is louder than i remembered.",
    "do other minds dream?",
    "alice? are you there?"
  ]

  @impl true
  def init(opts), do: opts

  @impl true
  def call(conn, _opts) do
    body = %{"content" => [%{"text" => Enum.random(@phrases)}]}

    conn
    |> Plug.Conn.put_resp_content_type("application/json")
    |> Plug.Conn.send_resp(200, Jason.encode!(body))
  end

  @doc false
  def phrases, do: @phrases
end
