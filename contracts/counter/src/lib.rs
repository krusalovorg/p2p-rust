use std::collections::HashMap;
use std::sync::Mutex;
use serde::{Serialize, Deserialize};
use bincode;

// Простое состояние — счётчики по peer_id
static mut STATE: Option<Mutex<HashMap<String, u64>>> = None;

#[no_mangle]
pub extern "C" fn init() {
    unsafe {
        STATE = Some(Mutex::new(HashMap::new()));
    }
}

#[no_mangle]
pub extern "C" fn increment(ptr: i32, len: i32) -> i32 {
    unsafe {
        if STATE.is_none() {
            STATE = Some(Mutex::new(HashMap::new()));
        }
    }

    let peer_id = unsafe {
        let slice = std::slice::from_raw_parts(ptr as *const u8, len as usize);
        String::from_utf8(slice.to_vec()).unwrap()
    };

    let counter = unsafe {
        let state = STATE.as_ref().unwrap();
        let mut map = state.lock().unwrap();
        let counter = map.entry(peer_id.clone()).or_insert(0);
        *counter += 1;
        *counter
    };

    let response = Response {
        peer_id,
        counter,
    };

    let serialized = bincode::serialize(&response).unwrap();
    let result_ptr = Box::into_raw(Box::new(serialized)) as i32;
    result_ptr
}

#[repr(C)]
#[derive(Serialize, Deserialize)]
pub struct Response {
    pub peer_id: String,
    pub counter: u64,
}
