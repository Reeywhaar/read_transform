# ReadTransformer

`ReadTransformer` is a struct for functional `Read` objects processing.

It takes `Read` object, map function and acts as medium `Read` object.

# Example

```rust
let mut data = Cursor::new(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
let mut transformed = ReadTransformer::new(
	&mut data,
	5,
	Box::new(|buffer: &mut [u8], _position, _last_attempt| -> Option<(Vec<u8>, usize)> {
		return Some((
			buffer
				.iter()
				.map(|x| {
					if x % 2 == 0 {
						return 0;
					};
					return *x;
				})
				.collect::<Vec<_>>(),
			buffer.len(),
		));
	}),
);
let mut out = vec![0; 10];
transformed.read_exact(&mut out).unwrap();
assert_eq!(out, [1, 0, 3, 0, 5, 0, 7, 0, 9, 0]);
```

# Usage
Add to your `Cargo.toml`s dependencies section:

```
read_transform = "0.1.0"
```

# Documentation
[Link](https://docs.rs/read_transform/0.1.0/read_transform/)

Also available with `cargo doc --no-deps --open` command