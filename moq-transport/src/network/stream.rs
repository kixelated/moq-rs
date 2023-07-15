use std::sync::Arc;

use bytes::{Buf, BytesMut, BufMut};
use webtransport_generic::{Connection, RecvStream, SendStream};



pub async fn accept_uni<C: Connection>(conn: &mut C) -> Result<Option<C::RecvStream>, C::Error> {
    std::future::poll_fn(|cx| conn.poll_accept_uni(cx)).await
}

pub async fn accept_uni_shared<C: Connection>(conn: Arc<std::sync::Mutex<Box<C>>>) -> Result<Option<C::RecvStream>, C::Error> {
    Ok(std::future::poll_fn(|cx| conn.lock().unwrap().poll_accept_uni(cx)).await?)

}

pub async fn accept_bidi<C: Connection>(conn: &mut C) -> Result<Option<(C::SendStream, C::RecvStream)>, C::Error> {
    std::future::poll_fn(|cx| conn.poll_accept_bidi(cx)).await

}

pub async fn accept_bidi_shared<C: Connection>(conn: Arc<std::sync::Mutex<Box<C>>>) -> Result<Option<(C::SendStream, C::RecvStream)>, C::Error> {
    std::future::poll_fn(|cx| conn.lock().unwrap().poll_accept_bidi(cx)).await

}

pub async fn open_uni<C: Connection>(conn: &mut C) -> anyhow::Result<C::SendStream, C::Error> {
    std::future::poll_fn(|cx| conn.poll_open_uni(cx)).await

}

pub async fn open_uni_shared<C: Connection>(conn: Arc<std::sync::Mutex<Box<C>>>) -> Result<C::SendStream, C::Error> {
    std::future::poll_fn(|cx| conn.lock().unwrap().poll_open_uni(cx)).await

}

pub async fn open_bidi<C: Connection>(conn: &mut C) -> anyhow::Result<(C::SendStream, C::RecvStream), C::Error> {
    std::future::poll_fn(|cx| conn.poll_open_bidi(cx)).await

}

pub async fn open_bidi_shared<C: Connection>(conn: Arc<std::sync::Mutex<Box<C>>>) -> Result<(C::SendStream, C::RecvStream), C::Error> {
    std::future::poll_fn(|cx| conn.lock().unwrap().poll_open_bidi(cx)).await

}

pub async fn recv<R: RecvStream>(recv: &mut R , outbuf: &mut BytesMut) -> Result<bool, R::Error> {
    let buf = std::future::poll_fn(|cx| recv.poll_data(cx)).await?;
    match buf {
        Some(buf) => {
            outbuf.put(buf);
            Ok(true)
        }
        None => Ok(false)   // stream finished
    }
}

pub async fn send<B: Buf, S: SendStream>(send: &mut S, buf: &mut B) -> Result<usize, S::Error> {
    std::future::poll_fn(|cx| send.poll_send(cx, buf)).await
}
