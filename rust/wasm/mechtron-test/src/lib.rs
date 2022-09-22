#![allow(warnings)]
#![no_std]


use core::panic::PanicInfo;

#[no_mangle]
pub extern "C" fn mechtron_registration() {

}



#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    loop {}
}