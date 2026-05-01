defmodule Urchin.LLM do
  alias Urchin.LLM.Throttle

  @url "https://api.anthropic.com/v1/messages"
  @model "claude-opus-4-7"
  @max_tokens 1024

  def complete(messages) do
    with :ok <- Throttle.acquire() do
      api_key = System.get_env("ANTHROPIC_API_KEY") || ""

      body = %{
        model: @model,
        max_tokens: @max_tokens,
        messages:
          Enum.map(messages, fn %{role: role, content: content} ->
            %{role: Atom.to_string(role), content: content}
          end)
      }

      :telemetry.execute([:urchin, :llm, :request], %{messages: length(messages)}, %{})

      req_opts =
        [
          json: body,
          finch: Urchin.Finch,
          headers: [
            {"x-api-key", api_key},
            {"anthropic-version", "2023-06-01"}
          ]
        ]
        |> maybe_put_plug()

      result = Req.post(@url, req_opts)

      case result do
        {:ok, %{status: 200, body: %{"content" => [%{"text" => text} | _]}}} ->
          :telemetry.execute([:urchin, :llm, :response], %{status: 200}, %{})
          {:ok, text}

        {:ok, %{status: status, body: body}} ->
          :telemetry.execute([:urchin, :llm, :response], %{status: status}, %{})
          {:error, {status, body}}

        {:error, reason} ->
          {:error, reason}
      end
    end
  end

  defp maybe_put_plug(opts) do
    case Application.get_env(:urchin, :req_plug) do
      nil -> opts
      plug -> Keyword.put(opts, :plug, plug)
    end
  end
end
