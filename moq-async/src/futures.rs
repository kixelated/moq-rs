use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

pub trait FuturesExt: Future {
	fn transpose(self) -> Transpose<Self>
	where
		Self: Sized,
	{
		Transpose { future: self }
	}

	fn cloned(self) -> Cloned<Self>
	where
		Self: Sized,
	{
		Cloned { future: self }
	}
}

impl<F: Future> FuturesExt for F {}

pub struct Transpose<F> {
	future: F,
}

impl<F, T, E> Future for Transpose<F>
where
	F: Future<Output = Result<Option<T>, E>>,
{
	type Output = Option<Result<T, E>>;

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		// Frankly I have no idea if this is correct; I hate Pin
		let future = unsafe { self.map_unchecked_mut(|s| &mut s.future) };

		match future.poll(cx) {
			Poll::Ready(Ok(Some(val))) => Poll::Ready(Some(Ok(val))),
			Poll::Ready(Ok(None)) => Poll::Ready(None),
			Poll::Ready(Err(err)) => Poll::Ready(Some(Err(err))),
			Poll::Pending => Poll::Pending,
		}
	}
}

pub struct Cloned<F> {
	future: F,
}

impl<F, T> Future for Cloned<F>
where
	F: Future<Output = T>,
	T: Clone,
{
	type Output = T;

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		// Frankly I have no idea if this is correct; I hate Pin
		let future = unsafe { self.map_unchecked_mut(|s| &mut s.future) };

		match future.poll(cx) {
			Poll::Ready(val) => Poll::Ready(val.clone()),
			Poll::Pending => Poll::Pending,
		}
	}
}
