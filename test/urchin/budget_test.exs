defmodule Urchin.BudgetTest do
  use ExUnit.Case, async: false

  alias Urchin.Budget

  setup do
    id = "mind-#{System.unique_integer([:positive])}"
    Budget.reset(id)
    %{id: id}
  end

  test "charge under the limit returns :ok and increments used", %{id: id} do
    assert :ok = Budget.charge(id)
    assert :ok = Budget.charge(id, 4)
    assert Budget.used(id) == 5
  end

  test "charge over the limit returns :budget_exceeded", %{id: id} do
    Application.put_env(:urchin, :budget_turns, 3)
    Budget.reset(id)

    assert :ok = Budget.charge(id)
    assert :ok = Budget.charge(id)
    assert :ok = Budget.charge(id)
    assert {:error, :budget_exceeded} = Budget.charge(id)
  after
    Application.delete_env(:urchin, :budget_turns)
  end

  test "reset zeroes the counter", %{id: id} do
    :ok = Budget.charge(id, 7)
    assert Budget.used(id) == 7
    :ok = Budget.reset(id)
    assert Budget.used(id) == 0
  end

  test "used/1 returns 0 for an unknown id" do
    assert Budget.used("never-charged-#{System.unique_integer([:positive])}") == 0
  end
end
