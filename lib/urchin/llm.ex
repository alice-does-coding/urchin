defmodule Urchin.LLM do
  @url "https://api.anthropic.com/v1/messages"
  @model "claude-opus-4-7"
  @max_tokens 1024

  def complete(messages) do
    api_key = System.fetch_env!("ANTHROPIC_API_KEY")

    body = %{
      model: @model,
      max_tokens: @max_tokens,
      messages: Enum.map(messages, fn %{role: role, content: content} ->
        %{role: Atom.to_string(role), content: content}
      end)
    }

    case Req.post(@url,
      json: body,
      headers: [
        {"x-api-key", api_key},
        {"anthropic-version", "2023-06-01"}
      ]
    ) do
      {:ok, %{status: 200, body: %{"content" => [%{"text" => text} | _]}}} ->
        {:ok, text}

      {:ok, %{status: status, body: body}} ->
        {:error, {status, body}}

      {:error, reason} ->
        {:error, reason}
    end
  end
end
