defmodule Urchin.Dev.LLMStubTest do
  use ExUnit.Case, async: true

  alias Urchin.Dev.LLMStub

  test "returns 200 with a content array shaped like the Anthropic API" do
    conn = Plug.Test.conn(:post, "/", "{}")
    response = LLMStub.call(conn, [])

    assert response.status == 200
    body = Jason.decode!(response.resp_body)
    assert [%{"text" => text}] = body["content"]
    assert is_binary(text)
    assert text in LLMStub.phrases()
  end

  test "sets application/json content-type" do
    conn = Plug.Test.conn(:post, "/", "{}")
    response = LLMStub.call(conn, [])

    assert Plug.Conn.get_resp_header(response, "content-type") == [
             "application/json; charset=utf-8"
           ]
  end

  test "integrates with Urchin.LLM.complete via :req_plug config" do
    Application.put_env(:urchin, :req_plug, LLMStub)
    on_exit(fn -> Application.delete_env(:urchin, :req_plug) end)

    assert {:ok, text} = Urchin.LLM.complete([%{role: :user, content: "hi"}])
    assert text in LLMStub.phrases()
  end
end
