use tokio::sync::mpsc::Receiver;
use crate::error::Error;

pub struct Progress<E>
{
    rx: Receiver<E>
}

impl <E> Progress<E>
{
}