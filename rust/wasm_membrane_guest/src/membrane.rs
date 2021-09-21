use std::sync::atomic::{Ordering,AtomicI32};
use std::sync::RwLock;
use std::collections::HashMap;

use wasm_bindgen::prelude::*;

use crate::error::Error;

lazy_static! {
  pub static ref BUFFERS: RwLock<HashMap<i32,Vec<u8>>> = RwLock::new(HashMap::new());
  pub static ref BUFFER_INDEX: AtomicI32 = AtomicI32::new(0);
}

pub static VERSION: i32 = 1;

extern "C"
{
    pub fn membrane_host_log(buffer: i32);
    pub fn membrane_host_panic(buffer: i32);
}

#[wasm_bindgen]
pub fn membrane_guest_version() -> i32
{
  VERSION
}

#[wasm_bindgen]
pub fn membrane_guest_alloc_buffer(len: i32) -> i32
{
    let buffer_id = BUFFER_INDEX.fetch_add(1, Ordering::Relaxed);
    {
        let mut buffers = BUFFERS.write().unwrap();
        let mut bytes: Vec<u8> = Vec::with_capacity(len as _);
        unsafe { bytes.set_len(len as _) }
        buffers.insert(buffer_id, bytes);
    }
    buffer_id
}

#[wasm_bindgen]
pub fn membrane_guest_dealloc_buffer(id: i32)
{
    let mut buffers = BUFFERS.write().unwrap();
    buffers.remove(&id);
}

#[wasm_bindgen]
pub fn membrane_guest_test(test_buffer_message: i32)
{
    log(membrane_consume_string(test_buffer_message).unwrap().as_str());
}

#[wasm_bindgen]
pub fn membrane_guest_get_buffer_ptr(id: i32) ->*const u8
{
    let buffer_info = BUFFERS.read();
    let buffer_info = buffer_info.unwrap();
    let buffer = buffer_info.get(&id).unwrap();
    return buffer.as_ptr()
}

#[wasm_bindgen]
pub fn membrane_guest_get_buffer_len(id: i32) ->i32
{
    let buffer_info = BUFFERS.read();
    let buffer_info = buffer_info.unwrap();
    let buffer = buffer_info.get(&id).unwrap();
    buffer.len() as _
}

#[wasm_bindgen]
pub fn membrane_guest_test_log(log_message_buffer: i32)
{
    let log_message = membrane_consume_string(log_message_buffer).unwrap();
    log( log_message.as_str() );
}


//////////////////////////////////////////////
// Convenience methods
//////////////////////////////////////////////

pub fn log(message: &str) {
    unsafe
    {
        let buffer = membrane_write_str(message);
        membrane_host_log(buffer);
    }
}

pub fn panic(message: &str)
{
    let buffer_id = membrane_write_str(message);
    unsafe {
        membrane_host_panic(buffer_id);
    }
}

pub fn membrane_write_buffer(bytes: Vec<u8>) -> i32 {
    let mut buffers = BUFFERS.write().unwrap();
    let buffer_id = BUFFER_INDEX.fetch_add(1, Ordering::Relaxed);
    buffers.insert(buffer_id, bytes);
    buffer_id
}


pub fn membrane_read_buffer(buffer: i32) -> Result<Vec<u8>, Error>
{
    let bytes = {
        let buffers = BUFFERS.read()?;
        buffers.get(&buffer).unwrap().clone()
    };
    Ok(bytes)
}

pub fn membrane_consume_buffer(buffer: i32) -> Result<Vec<u8>, Error>
{
    let bytes = {
        let mut buffers = BUFFERS.write()?;
        buffers.remove(&buffer).unwrap()
    };
    Ok(bytes)
}

pub fn membrane_read_string(buffer: i32) -> Result<String, Error>
{
    let bytes = membrane_read_buffer(buffer)?;
    let string = String::from_utf8(bytes)?;
    Ok(string)
}

pub fn membrane_consume_string(buffer: i32) -> Result<String, Error>
{
    let bytes = membrane_consume_buffer(buffer)?;
    let string = String::from_utf8(bytes)?;
    Ok(string)
}

pub fn membrane_write_str(string: &str) -> i32 {
    membrane_write_string(string.to_string())
}

pub fn membrane_write_string(mut string: String) -> i32 {
    let mut buffers = BUFFERS.write().unwrap();
    let buffer_id = BUFFER_INDEX.fetch_add(1, Ordering::Relaxed);
    unsafe {
        buffers.insert(buffer_id, string.as_mut_vec().to_vec());
    }
    buffer_id
}


