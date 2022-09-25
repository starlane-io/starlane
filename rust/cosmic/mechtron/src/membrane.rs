use cosmic_universe::err::UniErr;
use cosmic_universe::particle::Details;
use cosmic_universe::VERSION;
use cosmic_universe::wave::{Bounce, ReflectedAggregate, UltraWave};
use cosmic_universe::wave::exchange::synch::{DirectedHandlerProxy, DirectedHandlerShell};
use wasm_membrane_guest::membrane::{membrane_consume_buffer, membrane_consume_string, membrane_write_buffer};
use crate::{Guest, GUEST, MechtronFactories, synch};

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
