use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use dashmap::DashMap;
use cosmic_universe::err::UniErr;
use cosmic_universe::particle::Details;
use cosmic_universe::VERSION;
use cosmic_universe::wave::{Bounce, ReflectedAggregate, UltraWave};
use cosmic_universe::wave::exchange::synch::{DirectedHandlerProxy, DirectedHandlerShell};
use crate::{Guest, GUEST, MechtronFactories, synch};
use crate::err::MembraneErr;

#[no_mangle]
extern "C" {
    pub fn mechtron_frame_to_host(frame: i32) -> i32;
    pub fn mechtron_uuid() -> i32;
    pub fn mechtron_timestamp() -> i64;
    pub fn mechtron_register(factories: &mut MechtronFactories) -> Result<(), UniErr>;
}


#[no_mangle]
pub fn mechtron_guest_init(version: i32, frame: i32) -> i32 {
    let mut factories = MechtronFactories::new();
    unsafe {
        if let Err(_) = mechtron_register(&mut factories) {
            return -1;
        }
    }
    let version = membrane_consume_string(version).unwrap();
    if version != VERSION.to_string() {
        return -2;
    }
    let frame = membrane_consume_buffer(frame).unwrap();
    let details: Details = bincode::deserialize(frame.as_slice()).unwrap();

    {
        let mut write = GUEST.write().unwrap();
        let guest = synch::Guest::new(details, factories);
        write.replace(guest);
    }

    0
}

#[no_mangle]
pub fn mechtron_frame_to_guest(frame: i32) -> i32 {
    let frame = membrane_consume_buffer(frame).unwrap();
    let wave: UltraWave = bincode::deserialize(frame.as_slice()).unwrap();

    if wave.is_directed() {
        let wave = wave.to_directed().unwrap();
        let handler: DirectedHandlerShell<DirectedHandlerProxy> = {
            let read = GUEST.read().unwrap();
            let guest : &synch::Guest = read.as_ref().unwrap();
            guest.handler()
        };

        match handler.handle(wave) {
            Bounce::Absorbed => 0,
            Bounce::Reflected(wave) => {
                let wave = mechtron_write_wave_to_host(wave.to_ultra() ).unwrap();
                wave
            }
        }
    } else {
        // we simply do not deal with ReflectedWaves at this time
        // unless they are in the context of the same thread that made the request
        0
    }
}

pub fn mechtron_write_wave_to_host(wave: UltraWave) -> Result<i32, UniErr> {
    let data = bincode::serialize(&wave)?;
    Ok(membrane_write_buffer(data))
}

pub fn mechtron_exchange_wave_host<G>(wave: UltraWave) -> Result<ReflectedAggregate, G::Err> where G:Guest {
    let data = bincode::serialize(&wave)?;
    let buffer_id = membrane_write_buffer(data);
    let reflect_id = unsafe { mechtron_frame_to_host(buffer_id) };

    if reflect_id == 0 {
        Ok(ReflectedAggregate::None)
    } else {
        let buffer = membrane_consume_buffer(reflect_id)?;
        let agg: ReflectedAggregate = bincode::deserialize(buffer.as_slice())?;
        Ok(agg)
    }
}



lazy_static! {
    static ref BUFFERS: Arc<DashMap<i32, Vec<u8>>> = Arc::new(DashMap::new());
    static ref BUFFER_INDEX: AtomicI32 = AtomicI32::new(0);
}


#[no_mangle]
extern "C" {
    pub fn membrane_host_log(buffer: i32);
    pub fn membrane_host_panic(buffer: i32);
}

#[no_mangle]
pub extern "C" fn membrane_guest_version() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn membrane_guest_alloc_buffer(len: i32) -> i32 {
    let buffer_id = BUFFER_INDEX.fetch_add(1, Ordering::Relaxed);
    {
        let mut bytes: Vec<u8> = Vec::with_capacity(len as _);
        unsafe { bytes.set_len(len as _) }
        BUFFERS.insert(buffer_id, bytes);
    }
    buffer_id
}

#[no_mangle]
pub extern "C" fn membrane_guest_dealloc_buffer(id: i32) {
    BUFFERS.remove(&id);
}

#[no_mangle]
pub extern "C" fn membrane_guest_test(test_buffer_message: i32) {
    log(membrane_consume_string(test_buffer_message)
        .unwrap()
        .as_str());
}

#[no_mangle]
pub extern "C" fn membrane_guest_get_buffer_ptr(id: i32) -> *const u8 {
    let buffer = BUFFERS.get(&id).unwrap();
    return buffer.as_ptr();
}

#[no_mangle]
pub extern "C" fn membrane_guest_get_buffer_len(id: i32) -> i32 {
    let buffer = BUFFERS.get(&id).unwrap();
    buffer.len() as _
}

#[no_mangle]
pub extern "C" fn membrane_guest_test_log(log_message_buffer: i32) {
    let log_message = membrane_consume_string(log_message_buffer).unwrap();
    log(log_message.as_str());
}

//////////////////////////////////////////////
// Convenience methods
//////////////////////////////////////////////

pub fn log(message: &str) {
    unsafe {
        let buffer = membrane_write_str(message);
        membrane_host_log(buffer);
    }
}


pub fn membrane_write_buffer(bytes: Vec<u8>) -> i32 {
    let buffer_id = BUFFER_INDEX.fetch_add(1, Ordering::Relaxed);
    BUFFERS.insert(buffer_id, bytes);
    buffer_id
}

pub fn membrane_read_buffer(buffer: i32) -> Result<Vec<u8>, MembraneErr> {
    let bytes = {
        BUFFERS.get(&buffer).unwrap().clone()
    };
    Ok(bytes)
}

pub fn membrane_consume_buffer(buffer: i32) -> Result<Vec<u8>, MembraneErr> {
    let (_,bytes) = {
        BUFFERS.remove(&buffer).unwrap()
    };
    Ok(bytes)
}

pub fn membrane_read_string(buffer: i32) -> Result<String, MembraneErr> {
    let bytes = membrane_read_buffer(buffer)?;
    let string = String::from_utf8(bytes)?;
    Ok(string)
}

pub fn membrane_consume_string(buffer: i32) -> Result<String, MembraneErr> {
    let bytes = membrane_consume_buffer(buffer)?;
    let string = String::from_utf8(bytes)?;
    Ok(string)
}

pub fn membrane_write_str(string: &str) -> i32 {
    membrane_write_string(string.to_string())
}

pub fn membrane_write_string(mut string: String) -> i32 {
    let buffer_id = BUFFER_INDEX.fetch_add(1, Ordering::Relaxed);
    unsafe {
        BUFFERS.insert(buffer_id, string.as_mut_vec().to_vec());
    }
    buffer_id
}
