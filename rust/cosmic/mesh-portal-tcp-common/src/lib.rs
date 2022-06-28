#![allow(warnings)]

#[macro_use]
extern crate anyhow;


use std::convert::{TryFrom, TryInto};

use anyhow::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt, AsyncWrite};

use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use std::marker::PhantomData;
use std::time::Duration;
use mesh_portal::version::latest::frame::{PrimitiveFrame, CloseReason};
use mesh_portal::error;
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}

pub struct FrameWriter<FRAME> where FRAME: Serialize {
    stream: PrimitiveFrameWriter,
    phantom: PhantomData<FRAME>
}

impl <FRAME> FrameWriter<FRAME> where FRAME: Serialize  {
    pub fn new(stream: PrimitiveFrameWriter) -> Self {
        Self {
            stream,
            phantom: PhantomData
        }
    }

    pub fn done(self) -> PrimitiveFrameWriter {
        self.stream
    }
}

impl <F> FrameWriter<F> where F : Serialize  {

    pub async fn write( &mut self, frame: F ) -> Result<(),Error> {
        let data = bincode::serialize(&frame)?;
        let frame = PrimitiveFrame::from(data);
        self.stream.write(frame).await
    }

    pub async fn close( &mut self, reason: CloseReason ) {
//        self.write(outlet::Frame::Close(reason) ).await.unwrap_or_default();
    }

}

/*
impl FrameWriter<outlet::Frame>  {

    pub async fn write( &mut self, frame: outlet::Frame ) -> Result<(),Error> {
        let frame = frame.try_into()?;
        self.stream.write(frame).await
    }

    pub async fn close( &mut self, reason: CloseReason ) {
        self.write(outlet::Frame::Close(reason) ).await.unwrap_or_default();
    }

}

impl FrameWriter<inlet::Frame> {

    pub async fn write( &mut self, frame: inlet::Frame ) -> Result<(),Error> {
        let frame = frame.try_into()?;
        self.stream.write(frame).await
    }

    pub async fn close( &mut self, reason: CloseReason ) {
        self.write(inlet::Frame::Close(reason) ).await.unwrap_or_default();
    }
}

impl FrameWriter<initin::Frame> {

    pub async fn write( &mut self, frame: initin::Frame ) -> Result<(),Error> {
        let frame = frame.try_into()?;
        self.stream.write(frame).await
    }

    pub async fn close( &mut self, reason: CloseReason ) {
    }
}

impl FrameWriter<initout::Frame> {

    pub async fn write( &mut self, frame: initout::Frame ) -> Result<(),Error> {
        let frame = frame.try_into()?;
        self.stream.write(frame).await
    }

    pub async fn close( &mut self, reason: CloseReason ) {
    }
}

 */
pub struct FrameReader<FRAME> {
    stream: PrimitiveFrameReader,
    phantom: PhantomData<FRAME>
}

impl <FRAME> FrameReader<FRAME>  {
    pub fn new(stream: PrimitiveFrameReader) -> Self {
        Self {
            stream,
            phantom: PhantomData
        }
    }

    pub fn done(self) -> PrimitiveFrameReader {
        self.stream
    }
}

impl <F> FrameReader<F> where F: TryFrom<PrimitiveFrame,Error=error::MsgErr>{
    pub async fn read( &mut self ) -> Result<F,Error> {
        let frame = self.stream.read().await?;
        Ok(F::try_from(frame)?)
    }
}

/*
impl FrameReader<initin::Frame> {
    pub async fn read( &mut self ) -> Result<initin::Frame,Error> {
        let frame = self.stream.read().await?;
        Ok(bincode::deserialize(frame.data.as_slice())?)
    }
}

impl FrameReader<initout::Frame> {
    pub async fn read( &mut self ) -> Result<initout::Frame,Error> {
        let frame = self.stream.read().await?;
        Ok(initout::Frame::try_from(frame)?)
    }
}

impl FrameReader<outlet::Frame> {
    pub async fn read( &mut self ) -> Result<outlet::Frame,Error> {
        let frame = self.stream.read().await?;
        Ok(outlet::Frame::try_from(frame)?)
    }
}

impl FrameReader<inlet::Frame> {
    pub async fn read( &mut self ) -> Result<inlet::Frame,Error> {
        let frame = self.stream.read().await?;
        Ok(inlet::Frame::try_from(frame)?)
    }
}
 */

pub struct PrimitiveFrameReader {
    read: OwnedReadHalf
}

impl PrimitiveFrameReader {

    pub fn new(read: OwnedReadHalf ) -> Self {
        Self {
           read
        }
    }

    pub async fn read(&mut self) -> Result<PrimitiveFrame,Error> {
        let size = self.read.read_u32().await? as usize;

        let mut vec= vec![0 as u8; size];
        let buf = vec.as_mut_slice();
        self.read.read_exact(buf).await?;
        Result::Ok(PrimitiveFrame {
            data: vec
        })
    }

    pub async fn read_string(&mut self) -> Result<String,Error> {
        let frame = self.read().await?;
        Ok(frame.try_into()?)
    }

    pub fn done(self) -> OwnedReadHalf {
        self.read
    }

}

pub struct PrimitiveFrameWriter {
    write: OwnedWriteHalf,
}

impl PrimitiveFrameWriter {

    pub fn new(write: OwnedWriteHalf) -> Self {
        Self {
            write,
        }
    }


    pub async fn write( &mut self, frame: PrimitiveFrame ) -> Result<(),Error> {
        self.write.write_u32(frame.size() ).await?;
        self.write.write_all(frame.data.as_slice() ).await?;
        Ok(())
    }


    pub async fn write_string(&mut self, string: String) -> Result<(),Error> {
        let frame = PrimitiveFrame::from(string);
        self.write(frame).await
    }

    pub fn done(self) -> OwnedWriteHalf {
        self.write
    }
}



