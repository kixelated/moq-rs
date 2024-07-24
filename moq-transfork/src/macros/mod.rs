#[macro_export]
macro_rules! async_clone {
	($($var:ident),*, $block:block) => {
		{
			$(
				#[allow(unused_mut)]
				let mut $var = $var.clone();
			)*
			async move {
				$block
			}
		}
	};
}
