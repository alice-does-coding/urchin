defmodule Urchin.LLM.ThrottleTest do
  use ExUnit.Case, async: true

  alias Urchin.LLM.Throttle

  setup do
    name = :"throttle-#{System.unique_integer([:positive])}"
    pid = start_supervised!({Throttle, name: name, rate: 2, burst: 3, refill_ms: 100})
    %{name: name, pid: pid}
  end

  test "acquire returns :ok up to the burst then rejects", %{name: name} do
    assert :ok = Throttle.acquire(name)
    assert :ok = Throttle.acquire(name)
    assert :ok = Throttle.acquire(name)
    assert {:error, :rate_limited} = Throttle.acquire(name)
  end

  test "refill restores tokens up to the burst cap", %{name: name} do
    for _ <- 1..3, do: assert(:ok = Throttle.acquire(name))
    assert {:error, :rate_limited} = Throttle.acquire(name)

    Process.sleep(150)

    assert :ok = Throttle.acquire(name)
    assert :ok = Throttle.acquire(name)
    assert {:error, :rate_limited} = Throttle.acquire(name)
  end
end
