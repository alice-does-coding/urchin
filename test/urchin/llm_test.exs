defmodule Urchin.LLMTest do
  use ExUnit.Case, async: false

  alias Urchin.LLM

  setup do
    stub = :"urchin_llm_stub_#{System.unique_integer([:positive])}"
    Application.put_env(:urchin, :req_plug, {Req.Test, stub})
    on_exit(fn -> Application.delete_env(:urchin, :req_plug) end)
    %{stub: stub}
  end

  test "complete returns {:ok, text} on a 200 response", %{stub: stub} do
    Req.Test.stub(stub, fn conn ->
      Req.Test.json(conn, %{"content" => [%{"text" => "hello from claude"}]})
    end)

    assert {:ok, "hello from claude"} =
             LLM.complete([%{role: :user, content: "hi"}])
  end

  test "complete returns {:error, {status, body}} on a non-200", %{stub: stub} do
    Req.Test.stub(stub, fn conn ->
      conn
      |> Plug.Conn.put_status(429)
      |> Req.Test.json(%{"error" => "rate limited"})
    end)

    assert {:error, {429, %{"error" => "rate limited"}}} =
             LLM.complete([%{role: :user, content: "hi"}])
  end

  test "complete sends the model and messages in the request body", %{stub: stub} do
    test_pid = self()

    Req.Test.stub(stub, fn conn ->
      {:ok, body, conn} = Plug.Conn.read_body(conn)
      send(test_pid, {:body, Jason.decode!(body)})
      Req.Test.json(conn, %{"content" => [%{"text" => "ack"}]})
    end)

    {:ok, _} = LLM.complete([%{role: :user, content: "ping"}])

    assert_receive {:body, body}
    assert body["model"] == "claude-opus-4-7"
    assert [%{"role" => "user", "content" => "ping"}] = body["messages"]
  end

end
