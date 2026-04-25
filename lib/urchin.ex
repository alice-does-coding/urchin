defmodule Urchin do
  def spawn_mind(id) do
    DynamicSupervisor.start_child(Urchin.MindSupervisor, {Urchin.Mind, id})
  end
end
