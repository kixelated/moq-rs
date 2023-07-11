// Coming from https://github.com/hyperium/h3, the goal is to
// do a PR with the changes afterwards

use std::task::{self, Poll};
use bytes::{Buf, BufMut};
use anyhow::Error;

type ErrorCode = u64;
type StreamId = u64;

/// Trait representing a QUIC connection.
pub trait Connection {
    /// The type produced by `poll_accept_bidi()`
    type BidiStream: SendStream + RecvStream;
    /// The type of the sending part of `BidiStream`
    type SendStream: SendStream;
    /// The type produced by `poll_accept_recv()`
    type RecvStream: RecvStream;

    /// Accept an incoming unidirectional stream
    ///
    /// Returning `None` implies the connection is closing or closed.
    fn poll_accept_recv(
        &mut self,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<Option<Self::RecvStream>, Error>>;

    /// Accept an incoming bidirectional stream
    ///
    /// Returning `None` implies the connection is closing or closed.
    fn poll_accept_bidi(
        &mut self,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<Option<Self::BidiStream>, Error>>;

    /// Poll the connection to create a new bidirectional stream.
    fn poll_open_bidi(
        &mut self,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<Self::BidiStream, Error>>;

    /// Poll the connection to create a new unidirectional stream.
    fn poll_open_send(
        &mut self,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<Self::SendStream, Error>>;

    /// Close the connection immediately
    fn close(&mut self, code: ErrorCode, reason: &[u8]);
}

/// Trait for opening outgoing streams
pub trait OpenStreams {
    /// The type produced by `poll_open_bidi()`
    type BidiStream: SendStream + RecvStream;
    /// The type produced by `poll_open_send()`
    type SendStream: SendStream;
    /// The type of the receiving part of `BidiStream`
    type RecvStream: RecvStream;

    /// Poll the connection to create a new bidirectional stream.
    fn poll_open_bidi(
        &mut self,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<Self::BidiStream, Error>>;

    /// Poll the connection to create a new unidirectional stream.
    fn poll_open_send(
        &mut self,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<Self::SendStream, Error>>;

    /// Close the connection immediately
    fn close(&mut self, code: ErrorCode, reason: &[u8]);
}

/// A trait describing the "send" actions of a QUIC stream.
pub trait SendStream {

    /// Polls if the stream can send more data.
    fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), Error>>;

    /// Poll to finish the sending side of the stream.
    fn poll_finish(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), Error>>;

    /// Send a QUIC reset code.
    fn reset(&mut self, reset_code: u64);

    /// Get QUIC send stream id
    fn send_id(&self) -> StreamId;
}

/// Allows sending unframed pure bytes to a stream. Similar to [`AsyncWrite`](https://docs.rs/tokio/latest/tokio/io/trait.AsyncWrite.html)
pub trait SendStreamUnframed: SendStream {
    /// Attempts write data into the stream.
    ///
    /// Returns the number of bytes written.
    ///
    /// `buf` is advanced by the number of bytes written.
    fn poll_send<D: Buf>(
        &mut self,
        cx: &mut task::Context<'_>,
        buf: &mut D,
    ) -> Poll<Result<usize, Error>>;
}

/// A trait describing the "receive" actions of a QUIC stream.
pub trait RecvStream {
    /// The type of `Buf` for data received on this stream.
    type Buf: Buf + Send;

    /// Poll the stream for more data.
    ///
    /// When the receive side will no longer receive more data (such as because
    /// the peer closed their sending side), this should return `None`.
    fn poll_data(
        &mut self,
        cx: &mut task::Context<'_>,
    ) -> Poll<Result<Option<Self::Buf>, anyhow::Error>>;

    /// Send a `STOP_SENDING` QUIC code.
    fn stop_sending(&mut self, error_code: u64);

    /// Get QUIC send stream id
    fn recv_id(&self) -> StreamId;

}

pub async fn accept_recv<C: Connection>(conn: &mut C) -> anyhow::Result<Option<C::RecvStream>, Error> {
    Ok(std::future::poll_fn(|cx| conn.poll_accept_recv(cx)).await?)

}

pub async fn accept_bidi<C: Connection>(conn: &mut C) -> anyhow::Result<Option<C::BidiStream>, Error> {
    Ok(std::future::poll_fn(|cx| conn.poll_accept_bidi(cx)).await?)

}

pub async fn open_send<C: Connection>(conn: &mut C) -> anyhow::Result<C::SendStream, Error> {
    Ok(std::future::poll_fn(|cx| conn.poll_open_send(cx)).await?)

}

pub async fn open_bidi<C: Connection>(conn: &mut C) -> anyhow::Result<C::BidiStream, Error> {
    Ok(std::future::poll_fn(|cx| conn.poll_open_bidi(cx)).await?)

}



pub async fn recv<B: Buf, BM: BufMut, R: RecvStream<Buf = B>>(recv: &mut R , outbuf: &mut BM) -> anyhow::Result<bool> {
    let buf = std::future::poll_fn(|cx| recv.poll_data(cx)).await?;
    match buf {
        Some(buf) => {
            outbuf.put(buf);
            Ok(true)
        }
        None => Ok(false)   // stream finished
    }
}

pub async fn send<B: Buf, S: SendStreamUnframed>(send: &mut S, buf: &mut B) -> anyhow::Result<usize> {
    Ok(std::future::poll_fn(|cx| send.poll_send(cx, buf)).await?)
}



/// Optional trait to allow "splitting" a bidirectional stream into two sides.
pub trait BidiStream: SendStream + RecvStream {
    /// The type for the send half.
    type SendStream: SendStream;
    /// The type for the receive half.
    type RecvStream: RecvStream;

    /// Split this stream into two halves.
    fn split(self) -> (Self::SendStream, Self::RecvStream);
}