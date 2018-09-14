/*!
`ReadTransformer` is a struct for functional `Read` objects processing.

It takes `Read` object, map function and acts as medium `Read` object.

# Example

```ignore
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
*/

use std::cmp::min;
use std::io::{Error as IOError, ErrorKind as IOErrorKind, Read, Result as IOResult};

/// transform function which takes buffer and returns `Vec<u8>` and length of processed bytes
///
/// # Params
/// * `buffer` - u8 slice to process
/// * `position` - position (total number of processed bytes)
/// * `last_attempt` - will be true if EOF is reached, input buffer length is greater than zero, and previous call returned `None`. Indicates that this is last attempt before throwing error.
///
/// # Return
/// * function returns `Result` tuple with vector of processed bytes and length of bytes processed in input buffer. If function requires some more bytes to process succesfully it must return `None`.
///
/// ### Note about size in the function return
/// Size in the function return related to the input buffer and not output vector. For example if our function filters even bytes in `[1,2,3,4,5,6]` returned size must be `6` and not `3`.
pub type TransformFn = Box<FnMut(&mut [u8], usize, bool) -> Option<(Vec<u8>, usize)>>;

/// Transforms `Read` object with function
pub struct ReadTransformer<T: Read> {
	input: T,
	buffer: Vec<u8>,
	position: usize,
	read: usize,
	residue: Vec<u8>,
	transform: TransformFn,
}

impl<T: Read> ReadTransformer<T> {
	/// Creates new `ReadTransformer`
	///
	/// # Params
	/// * `input` - input which will be processed
	/// * `size` - size of intermediate buffer
	/// * `transform_fn` - boxed function which acts like a map function.
	pub fn new(input: T, size: usize, transform_fn: TransformFn) -> Self {
		Self {
			input,
			buffer: vec![0; size],
			position: 0,
			read: 0,
			residue: vec![],
			transform: transform_fn,
		}
	}
}

impl<T: Read> Read for ReadTransformer<T> {
	fn read(&mut self, buffer: &mut [u8]) -> IOResult<usize> {
		if !self.residue.is_empty() {
			let len = min(self.residue.len(), buffer.len());
			buffer[..len].copy_from_slice(&self.residue[..len]);
			self.residue.drain(..len);
			return Ok(len);
		};
		loop {
			let read = self.input.read(&mut self.buffer[self.read..])?;
			self.read += read;
			if self.read == 0 {
				return Ok(0);
			};
			let mut res = (self.transform)(&mut self.buffer[..self.read], self.position, false);
			if res.is_none() && read == 0 {
				res = (self.transform)(&mut self.buffer[..self.read], self.position, true);
			}
			if res.is_none() {
				if read == 0 {
					return Err(IOError::new(
						IOErrorKind::Other,
						"EOF reached and the length of the buffer is less than transform function accepts to process"
					));
				};
				if self.read == self.buffer.len() {
					return Err(IOError::new(
						IOErrorKind::Other,
						"Intermediate buffer length is less than transform function accepts to process"
					));
				};
				continue;
			} else {
				let (mut output, processed) = res.unwrap();
				if output.is_empty() {
					self.read -= processed;
					self.position = self.position.wrapping_add(processed);
					continue;
				};
				let len = min(output.len(), buffer.len());
				buffer[..len].copy_from_slice(&output[..len]);
				output.drain(..len);
				self.residue = output;
				self.buffer[..].rotate_left(processed);
				self.read -= processed;
				self.position = self.position.wrapping_add(processed);
				return Ok(len);
			}
		}
	}
}

/// Convenience trait which implemented by all `Read` objects. Allows chaining of `Read` objects.
///
/// # Example
/// ```ignore
/// let mut data = Cursor::new(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]).transform(
/// 	5,
/// 	Box::new(|buffer: &mut [u8], _position, _last_attempt| -> Option<(Vec<u8>, usize)> {
/// 		let buf = buffer
/// 			.iter()
/// 			.filter(|x| {
/// 				return *x % 2 == 0;
/// 			})
/// 			.cloned()
/// 			.collect::<Vec<_>>();
/// 		return Some((buf, buffer.len()));
/// 	}),
/// );
/// let mut out = vec![0; 5];
/// data.read_exact(&mut out).unwrap();
/// assert_eq!(out, [2, 4, 6, 8, 10]);
/// ```
pub trait TransformableRead<T: Read>: Read {
	fn transform(self, buffer_size: usize, transform_fn: TransformFn) -> ReadTransformer<T>;
	fn transform_by_tuple(self, (usize, TransformFn)) -> ReadTransformer<T>;
}

impl<T: Read> TransformableRead<T> for T {
	fn transform(self, buffer_size: usize, transform_fn: TransformFn) -> ReadTransformer<T> {
		ReadTransformer::new(self, buffer_size, transform_fn)
	}
	fn transform_by_tuple(self, tuple: (usize, TransformFn)) -> ReadTransformer<T> {
		ReadTransformer::new(self, tuple.0, tuple.1)
	}
}

#[cfg(test)]
mod read_transformer_tests {
	use super::{ReadTransformer, TransformableRead};
	use std::io::{Cursor, Read};

	#[test]
	fn even_zeroed_test() {
		let mut data = Cursor::new(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
		let mut transformed = ReadTransformer::new(
			&mut data,
			5,
			Box::new(|buffer: &mut [u8], _, _| -> Option<(Vec<u8>, usize)> {
				return Some((
					buffer
						.iter()
						.map(|x| {
							if x % 2 == 0 {
								return 0;
							};
							return *x;
						}).collect::<Vec<_>>(),
					buffer.len(),
				));
			}),
		);
		let mut out = vec![0; 10];
		transformed.read_exact(&mut out).unwrap();
		assert_eq!(out, [1, 0, 3, 0, 5, 0, 7, 0, 9, 0]);
	}

	#[test]
	fn filter_test() {
		let mut data = Cursor::new(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
		let mut transformed = ReadTransformer::new(
			&mut data,
			5,
			Box::new(|buffer: &mut [u8], _, _| -> Option<(Vec<u8>, usize)> {
				let buf = buffer
					.iter()
					.filter(|x| {
						return *x % 2 == 0;
					}).cloned()
					.collect::<Vec<_>>();
				return Some((buf, buffer.len()));
			}),
		);
		let mut out = vec![0; 5];
		transformed.read_exact(&mut out).unwrap();
		assert_eq!(out, [2, 4, 6, 8, 10]);
	}

	#[test]
	fn reverse_test() {
		let mut data = Cursor::new(vec![1, 2, 3, 4, 5, 6, 7, 8]);
		let mut transformed = ReadTransformer::new(
			&mut data,
			6,
			Box::new(|buffer: &mut [u8], _, _| -> Option<(Vec<u8>, usize)> {
				if buffer.len() < 4 {
					return None;
				}
				let mut out = buffer[..4].to_vec().clone();
				out.reverse();
				return Some((out, 4));
			}),
		);
		let mut out = vec![0; 8];
		transformed.read_exact(&mut out).unwrap();
		assert_eq!(out, [4, 3, 2, 1, 8, 7, 6, 5]);
	}

	#[test]
	fn combined_test() {
		let mut data = Cursor::new(vec![1, 2, 3, 4, 5, 6, 7, 8]);
		let mut transformed = ReadTransformer::new(
			&mut data,
			4,
			Box::new(|buffer: &mut [u8], _, _| -> Option<(Vec<u8>, usize)> {
				if buffer.len() < 4 {
					return None;
				}
				let mut out = buffer.to_vec().clone();
				out.reverse();
				return Some((out, 4));
			}),
		);
		let mut transformed = ReadTransformer::new(
			&mut transformed,
			2,
			Box::new(|buffer: &mut [u8], _, _| -> Option<(Vec<u8>, usize)> {
				if buffer.len() < 2 {
					return None;
				}
				let mut out = buffer.to_vec().clone();
				out.reverse();
				return Some((out, 2));
			}),
		);
		let mut out = vec![0; 8];
		transformed.read_exact(&mut out).unwrap();
		assert_eq!(out, [3, 4, 1, 2, 7, 8, 5, 6]);
	}

	#[test]
	fn closure_test() {
		let mut data = Cursor::new(vec![1, 2, 3, 4, 5, 6, 7, 8]);
		let mut i = 0;
		let mut transformed = ReadTransformer::new(
			&mut data,
			4,
			Box::new(move |buffer: &mut [u8], _, _| -> Option<(Vec<u8>, usize)> {
				let out = buffer
					.to_vec()
					.iter()
					.map(|x| {
						let x = x + i;
						i += 1;
						return x;
					}).collect::<Vec<_>>();
				return Some((out, buffer.len()));
			}),
		);
		let mut out = vec![0; 8];
		transformed.read_exact(&mut out).unwrap();
		assert_eq!(out, [1, 3, 5, 7, 9, 11, 13, 15]);
	}

	#[test]
	fn transformable_read_test() {
		let mut data = Cursor::new(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]).transform(
			5,
			Box::new(|buffer: &mut [u8], _, _| -> Option<(Vec<u8>, usize)> {
				let buf = buffer
					.iter()
					.filter(|x| {
						return *x % 2 == 0;
					}).cloned()
					.collect::<Vec<_>>();
				return Some((buf, buffer.len()));
			}),
		);
		let mut out = vec![0; 5];
		data.read_exact(&mut out).unwrap();
		assert_eq!(out, [2, 4, 6, 8, 10]);
	}

	#[test]
	fn transformable_read_mut_test() {
		let mut data = Cursor::new(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
		let mut transformed = (&mut data).transform(
			5,
			Box::new(|buffer: &mut [u8], _, _| -> Option<(Vec<u8>, usize)> {
				let buf = buffer
					.iter()
					.filter(|x| {
						return *x % 2 == 0;
					}).cloned()
					.collect::<Vec<_>>();
				return Some((buf, buffer.len()));
			}),
		);
		let mut out = vec![0; 5];
		transformed.read_exact(&mut out).unwrap();
		assert_eq!(out, [2, 4, 6, 8, 10]);
	}
}
