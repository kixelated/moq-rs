use crate::Error;

pub trait Run {
    async fn run(&mut self) -> Result<(), Error>;
}

impl<T: Run> Run for Option<T> {
    async fn run(&mut self) -> Result<(), Error> {
        match self.as_mut() {
            Some(inner) => inner.run().await,
            None => Ok(()),
        }
    }
}
