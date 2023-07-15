use h3::quic::SendStream;
use h3::quic::RecvStream;
use h3::quic::SendStreamUnframed;

pub struct QuinnSendStream {
    stream: h3_webtransport::stream::SendStream<h3_quinn::SendStream<bytes::Bytes>, bytes::Bytes>
}

impl QuinnSendStream {
    pub fn new(stream: h3_webtransport::stream::SendStream<h3_quinn::SendStream<bytes::Bytes>, bytes::Bytes>) -> QuinnSendStream {
        QuinnSendStream { stream }
    }
}

impl webtransport_generic::SendStream for QuinnSendStream {

    type Error = anyhow::Error;

    fn poll_finish(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        self.stream.poll_finish(cx).map_err(|e| anyhow::anyhow!("{:?}", e))
    }

    fn reset(&mut self, reset_code: u32) {
        self.stream.reset(reset_code as u64)
    }

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

impl webtransport_generic::RecvStream for QuinnRecvStream {
    type Error = anyhow::Error;
    type Buf = bytes::Bytes;

    fn poll_data(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<Option<Self::Buf>, Self::Error>> {
        self.stream.poll_data(cx).map_err(|e| anyhow::anyhow!("{:?}", e))
    }

    fn stop_sending(&mut self, error_code: u32) {
        self.stream.stop_sending(error_code as u64)
    }
}
