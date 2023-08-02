use super::watch;
use bytes::Bytes;

pub type Publisher = watch::Publisher<Bytes>;
pub type Subscriber = watch::Subscriber<Bytes>;
