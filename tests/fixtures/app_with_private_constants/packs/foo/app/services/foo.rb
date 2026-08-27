module Foo
  def references_private_constants
    [
      Bar::Private,
      Bar::Private::Nested,
      Bar::Other,
    ]
  end
end
