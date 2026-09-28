use pinapod::PinaPod;

#[derive(PinaPod)]
#[pinapod(compact, crate = pinapod)]
struct ArrayNestedPrefixWidth {
	// The element-wise mapping would silently re-encode this as
	// `PodString<8, 1>`, changing the declared wire layout, so the derive
	// must reject the declaration instead.
	values: [pinapod::PodString<8, 3>; 4],
}

fn main() {}
