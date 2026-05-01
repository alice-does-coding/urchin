defmodule Urchin do
  def spawn_mind(id) do
    Horde.DynamicSupervisor.start_child(Urchin.MindSupervisor, Urchin.Mind.child_spec(id))
  end
end
