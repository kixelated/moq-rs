use h3::quic::SendStream;
use h3::quic::RecvStream;
use h3::quic::BidiStream;
use h3::quic::SendStreamUnframed;

use super::stream_id_to_u64;

pub struct QuinnSendStream {
    stream: h3_webtransport::stream::SendStream<h3_quinn::SendStream<bytes::Bytes>, bytes::Bytes>
}

impl QuinnSendStream {
    pub fn new(stream: h3_webtransport::stream::SendStream<h3_quinn::SendStream<bytes::Bytes>, bytes::Bytes>) -> QuinnSendStream {
        QuinnSendStream { stream }
    }
}

impl moq_generic_transport::SendStream for QuinnSendStream {
    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), anyhow::Error>> {
        self.stream.poll_ready(cx).map_err(|e| anyhow::anyhow!("{:?}", e))
    }

    fn poll_finish(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), anyhow::Error>> {
        self.stream.poll_finish(cx).map_err(|e| anyhow::anyhow!("{:?}", e))
    }

    fn reset(&mut self, reset_code: u64) {
        self.stream.reset(reset_code)
    }

    fn send_id(&self) -> u64 {
        stream_id_to_u64(self.stream.send_id())
    }
}

impl moq_generic_transport::SendStreamUnframed for QuinnSendStream {
    fn poll_send<D: bytes::Buf>(
        &mut self,
        cx: &mut std::task::Context<'_>,
        buf: &mut D,
    ) -> std::task::Poll<Result<usize, anyhow::Error>> {
        self.stream.poll_send(cx, buf).map_err(|e| anyhow::anyhow!("{:?}", e))
    }
}


pub struct QuinnRecvStream {
    stream: h3_webtransport::stream::RecvStream<h3_quinn::RecvStream, bytes::Bytes>
}

impl QuinnRecvStream {
    pub fn new(stream: h3_webtransport::stream::RecvStream<h3_quinn::RecvStream, bytes::Bytes>) -> QuinnRecvStream {
        QuinnRecvStream { stream }
    }
}

impl moq_generic_transport::RecvStream for QuinnRecvStream {
    type Buf = bytes::Bytes;

    fn poll_data(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<Option<Self::Buf>, anyhow::Error>> {
        self.stream.poll_data(cx).map_err(|e| anyhow::anyhow!("{:?}", e))
    }

    fn stop_sending(&mut self, error_code: u64) {
        self.stream.stop_sending(error_code)
    }

    fn recv_id(&self) -> u64 {
        stream_id_to_u64(self.stream.recv_id())
    }
}

pub struct QuinnBidiStream {
    stream: h3_webtransport::stream::BidiStream<h3_quinn::BidiStream<bytes::Bytes>, bytes::Bytes>
}

impl QuinnBidiStream {
    pub fn new(stream: h3_webtransport::stream::BidiStream<h3_quinn::BidiStream<bytes::Bytes>, bytes::Bytes>) -> QuinnBidiStream {
        QuinnBidiStream { stream }
    }
}

impl moq_generic_transport::BidiStream for QuinnBidiStream {
    type SendStream = QuinnSendStream;

    type RecvStream = QuinnRecvStream;

    fn split(self) -> (Self::SendStream, Self::RecvStream) {
        let (send, recv) = self.stream.split();
        let send = QuinnSendStream{
            stream: send,
        };
        let recv = QuinnRecvStream{
            stream: recv,
        };
        (send, recv)
    }
}

impl moq_generic_transport::RecvStream for QuinnBidiStream {
    type Buf = bytes::Bytes;

    fn poll_data(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<Option<Self::Buf>, anyhow::Error>> {
        self.stream.poll_data(cx).map_err(|e| anyhow::anyhow!("{:?}", e))
    }

    fn stop_sending(&mut self, error_code: u64) {
        self.stream.stop_sending(error_code)
    }

    fn recv_id(&self) -> u64 {
        stream_id_to_u64(self.stream.recv_id())
    }
}

impl moq_generic_transport::SendStream for QuinnBidiStream {
    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), anyhow::Error>> {
        self.stream.poll_ready(cx).map_err(|e| anyhow::anyhow!("{:?}", e))
    }

    fn poll_finish(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), anyhow::Error>> {
        self.stream.poll_finish(cx).map_err(|e| anyhow::anyhow!("{:?}", e))
    }

    fn reset(&mut self, reset_code: u64) {
        self.stream.reset(reset_code)
    }

    fn send_id(&self) -> u64 {
        stream_id_to_u64(self.stream.send_id())
    }
}

impl moq_generic_transport::SendStreamUnframed for QuinnBidiStream {
    fn poll_send<D: bytes::Buf>(
        &mut self,
        cx: &mut std::task::Context<'_>,
        buf: &mut D,
    ) -> std::task::Poll<Result<usize, anyhow::Error>> {
        self.stream.poll_send(cx, buf).map_err(|e| anyhow::anyhow!("{:?}", e))
    }
}

