#![no_main]
#[macro_use]
extern crate libfuzzer_sys;
extern crate ctlv;

use ctlv::CtlvRef;

fuzz_target!(|data: &[u8]| {
    // test that roundtrips work
    match CtlvRef::decode(data) {
        Err(_) => {}
        Ok((ctlv, len)) => {
            let mut enc = Vec::with_capacity(len);
            enc.resize(len, 0);
            assert_eq!(ctlv.encode(&mut enc[..]), len);
            assert_eq!(&enc[..], &data[..len]);
        }
    }
});
