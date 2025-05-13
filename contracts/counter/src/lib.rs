use std::collections::HashMap;
use std::sync::Mutex;
use serde::{Serialize, Deserialize};

// Простое состояние — счётчики по peer_id
static mut STATE: Option<Mutex<HashMap<String, u64>>> = None;

#[no_mangle]
pub extern "C" fn init() {
    unsafe {
        STATE = Some(Mutex::new(HashMap::new()));
    }
}

#[no_mangle]
pub extern "C" fn increment(ptr: *const u8, len: usize) -> u64 {
    let peer_id = unsafe {
        let slice = std::slice::from_raw_parts(ptr, len);
        String::from_utf8(slice.to_vec()).unwrap()
    };

    unsafe {
        let state = STATE.as_ref().unwrap();
        let mut map = state.lock().unwrap();
        let counter = map.entry(peer_id).or_insert(0);
        *counter += 1;
        *counter
    }
}

#[repr(C)]
#[derive(Serialize, Deserialize)]
pub struct Response {
    pub peer_id: String,
    pub counter: u64,
}
