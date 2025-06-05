use std::collections::HashMap;
use std::sync::Mutex;
use serde::{Serialize, Deserialize};
use bincode;

#[derive(Serialize, Deserialize, Clone)]
pub struct User {
    pub telegram_id: String,
    pub nickname: String,
    pub firstname: String,
    pub lastname: String,
    pub balance: u64,
}

#[derive(Serialize, Deserialize)]
pub struct TransferRequest {
    pub from_telegram_id: String,
    pub to_telegram_id: String,
    pub amount: u64,
}

#[derive(Serialize, Deserialize)]
pub struct Response {
    pub success: bool,
    pub message: String,
    pub data: Option<User>,
}

static mut STATE: Option<Mutex<HashMap<String, User>>> = None;

#[no_mangle]
pub extern "C" fn init() {
    unsafe {
        STATE = Some(Mutex::new(HashMap::new()));
    }
}

#[no_mangle]
pub extern "C" fn register_user(ptr: i32, len: i32) -> i32 {
    unsafe {
        if STATE.is_none() {
            STATE = Some(Mutex::new(HashMap::new()));
        }
    }

    let user: User = unsafe {
        let slice = std::slice::from_raw_parts(ptr as *const u8, len as usize);
        bincode::deserialize(slice).unwrap()
    };

    let response = unsafe {
        let state = STATE.as_ref().unwrap();
        let mut map = state.lock().unwrap();
        
        if map.contains_key(&user.telegram_id) {
            Response {
                success: false,
                message: "User already exists".to_string(),
                data: None,
            }
        } else {
            map.insert(user.telegram_id.clone(), user.clone());
            Response {
                success: true,
                message: "User registered successfully".to_string(),
                data: Some(user),
            }
        }
    };

    let serialized = bincode::serialize(&response).unwrap();
    let result_ptr = Box::into_raw(Box::new(serialized)) as i32;
    result_ptr
}

#[no_mangle]
pub extern "C" fn transfer_tokens(ptr: i32, len: i32) -> i32 {
    unsafe {
        if STATE.is_none() {
            STATE = Some(Mutex::new(HashMap::new()));
        }
    }

    let transfer: TransferRequest = unsafe {
        let slice = std::slice::from_raw_parts(ptr as *const u8, len as usize);
        bincode::deserialize(slice).unwrap()
    };

    let response = unsafe {
        let state = STATE.as_ref().unwrap();
        let mut map = state.lock().unwrap();
        
        if !map.contains_key(&transfer.from_telegram_id) {
            return create_response(false, "Sender not found", None);
        }
        
        if !map.contains_key(&transfer.to_telegram_id) {
            return create_response(false, "Recipient not found", None);
        }

        let from_user = map.get_mut(&transfer.from_telegram_id).unwrap();
        if from_user.balance < transfer.amount {
            return create_response(false, "Insufficient balance", None);
        }

        from_user.balance -= transfer.amount;
        let to_user = map.get_mut(&transfer.to_telegram_id).unwrap();
        to_user.balance += transfer.amount;

        Response {
            success: true,
            message: "Transfer successful".to_string(),
            data: Some(to_user.clone()),
        }
    };

    let serialized = bincode::serialize(&response).unwrap();
    let result_ptr = Box::into_raw(Box::new(serialized)) as i32;
    result_ptr
}

fn create_response(success: bool, message: &str, data: Option<User>) -> i32 {
    let response = Response {
        success,
        message: message.to_string(),
        data,
    };
    let serialized = bincode::serialize(&response).unwrap();
    let result_ptr = Box::into_raw(Box::new(serialized)) as i32;
    result_ptr
}
