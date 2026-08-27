module Foo
  def self.calls_the_neighbours
    Bar.new
    Quux.new
  end
end
