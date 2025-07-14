use crate::connection::Connection;
use crate::db::P2PDatabase;
use crate::logger::{debug, error};
use crate::manager::ConnectionManager::ConnectionManager;
use crate::packets::TransportPacket;
use dashmap::DashMap;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct PythonPluginManager {
    plugins: Arc<Mutex<Vec<PyObject>>>,
    db: Arc<P2PDatabase>,
    manager: Arc<ConnectionManager>,
}

impl PythonPluginManager {
    pub fn new(
        db: Arc<P2PDatabase>,
        manager: Arc<ConnectionManager>,
    ) -> PyResult<Self> {
        // Устанавливаем PYTHONPATH
        let current_dir = std::env::current_dir().unwrap();
        std::env::set_var("PYTHONPATH", format!("{};{}", 
            current_dir.to_str().unwrap(),
            current_dir.join("modules").to_str().unwrap()
        ));

        Python::with_gil(|py| {
            let sys = py.import("sys")?;
            let path = sys.getattr("path")?;
            path.call_method1("append", ("modules",))?;

            // Инициализируем модуль p2p_rust
            let p2p_rust = PyModule::new(py, "p2p_rust")?;
            p2p_rust.add_function(wrap_pyfunction!(get_peer_id, p2p_rust.clone())?)?;
            p2p_rust.add_function(wrap_pyfunction!(get_storage_fragments, p2p_rust.clone())?)?;
            p2p_rust.add_function(wrap_pyfunction!(get_peer_with_most_space, p2p_rust.clone())?)?;
            p2p_rust.add_function(wrap_pyfunction!(get_token, p2p_rust.clone())?)?;
            p2p_rust.add_function(wrap_pyfunction!(get_contract_metadata, p2p_rust.clone())?)?;
            
            // Добавляем модуль в sys.modules
            let modules = sys.getattr("modules")?;
            modules.set_item("p2p_rust", p2p_rust.clone())?;

            // Создаем глобальные переменные для Python
            let globals = PyDict::new(py);
            globals.set_item("p2p_rust", p2p_rust)?;
            globals.set_item("__builtins__", py.import("builtins")?)?;

            debug("Initialized p2p_rust module");

            Ok(Self {
                plugins: Arc::new(Mutex::new(Vec::new())),
                db,
                manager,
            })
        })
    }

    pub async fn load_plugins(&self) -> PyResult<()> {
        let plugins_dir = Path::new("modules");
        if !plugins_dir.exists() {
            std::fs::create_dir_all(plugins_dir)?;
        }

        for entry in std::fs::read_dir(plugins_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("py") {
                self.load_plugin(&path).await?;
            }
        }
        Ok(())
    }

    async fn load_plugin(&self, path: &Path) -> PyResult<()> {
        let process_fn = Python::with_gil(|py| {
            let module_name = path.file_stem().unwrap().to_str().unwrap();
            let code = std::fs::read_to_string(path)?;

            // Создаем словарь для глобальных переменных
            let globals = PyDict::new(py);
            
            // Добавляем необходимые модули в глобальные переменные
            globals.set_item("p2p_rust", py.import("p2p_rust")?)?;
            globals.set_item("json", py.import("json")?)?;
            globals.set_item("sys", py.import("sys")?)?;
            globals.set_item("__builtins__", py.import("builtins")?)?;

            // Выполняем код плагина с нашими глобальными переменными
            let locals = PyDict::new(py);
            let code_cstr = std::ffi::CString::new(code.as_bytes()).unwrap();
            py.run(code_cstr.as_ref(), Some(&globals), Some(&locals))?;

            // Получаем функцию process_packet из локальных переменных
            if !locals.contains("process_packet")? {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "Plugin {} must have process_packet function",
                    module_name
                )));
            }

            let process_fn = locals.get_item("process_packet").unwrap().unwrap().extract::<PyObject>()?;
            debug(&format!("Loaded plugin: {}", module_name));
            Ok(process_fn)
        })?;

        // Добавляем функцию в список плагинов асинхронно
        self.plugins.lock().await.push(process_fn);
        Ok(())
    }

    pub async fn process_packet(&self, packet: &TransportPacket) -> Option<TransportPacket> {
        let packet_json = serde_json::to_string(packet).unwrap();
        
        Python::with_gil(|py| {
            let plugins = self.plugins.try_lock().unwrap();
            
            for plugin in plugins.iter() {
                match plugin.call1(py, (packet_json.clone(),)) {
                    Ok(result) => {
                        if result.is_none(py) {
                            debug("Plugin blocked packet");
                            return None;
                        }
                        
                        if let Ok(json_str) = result.extract::<String>(py) {
                            if let Ok(modified_packet) = serde_json::from_str::<TransportPacket>(&json_str) {
                                return Some(modified_packet);
                            }
                        }
                    }
                    Err(e) => {
                        error(&format!("Plugin error: {}", e));
                    }
                }
            }
            Some(packet.clone())
        })
    }
}

// Экспортируем функции Rust в Python
#[pymodule]
fn p2p_rust(_py: Python, m: &Bound<PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(get_peer_id, m)?)?;
    m.add_function(wrap_pyfunction!(get_storage_fragments, m)?)?;
    m.add_function(wrap_pyfunction!(get_peer_with_most_space, m)?)?;
    m.add_function(wrap_pyfunction!(get_token, m)?)?;
    m.add_function(wrap_pyfunction!(get_contract_metadata, m)?)?;
    Ok(())
}

#[pyfunction]
fn get_peer_id() -> PyResult<String> {
    let manager = PythonPluginManager::get_instance()?;
    let db = &manager.db;

    match db.get_or_create_peer_id() {
        Ok(peer_id) => Ok(peer_id),
        Err(e) => Err(pyo3::exceptions::PyValueError::new_err(e.to_string())),
    }
}

#[pyfunction]
fn get_storage_fragments() -> PyResult<String> {
    let manager = PythonPluginManager::get_instance()?;
    let db = &manager.db;

    match db.get_storage_fragments() {
        Ok(fragments) => Ok(json!(fragments).to_string()),
        Err(e) => Err(pyo3::exceptions::PyValueError::new_err(e.to_string())),
    }
}

#[pyfunction]
fn get_peer_with_most_space() -> PyResult<String> {
    let manager = PythonPluginManager::get_instance()?;
    let db = &manager.db;

    match db.get_peer_with_most_space() {
        Some(peer_id) => Ok(json!({ "peer_id": peer_id }).to_string()),
        None => Ok(json!({ "peer_id": null }).to_string()),
    }
}

#[pyfunction]
fn get_token(peer_id: &str) -> PyResult<String> {
    let manager = PythonPluginManager::get_instance()?;
    let db = &manager.db;

    match db.get_token(peer_id) {
        Ok(Some(token_info)) => Ok(json!(token_info).to_string()),
        Ok(None) => Ok(json!({ "token": null }).to_string()),
        Err(e) => Err(pyo3::exceptions::PyValueError::new_err(e.to_string())),
    }
}

#[pyfunction]
fn get_contract_metadata(contract_id: &str) -> PyResult<String> {
    let manager = PythonPluginManager::get_instance()?;
    let db = &manager.db;

    match db.get_contract_metadata(contract_id) {
        Ok(Some(metadata)) => Ok(json!(metadata).to_string()),
        Ok(None) => Ok(json!({ "metadata": null }).to_string()),
        Err(e) => Err(pyo3::exceptions::PyValueError::new_err(e.to_string())),
    }
}

// Глобальный экземпляр менеджера плагинов
static mut PLUGIN_MANAGER: Option<PythonPluginManager> = None;

impl PythonPluginManager {
    pub fn set_instance(manager: PythonPluginManager) {
        unsafe {
            PLUGIN_MANAGER = Some(manager);
        }
    }

    pub fn get_instance() -> PyResult<&'static PythonPluginManager> {
        unsafe {
            PLUGIN_MANAGER.as_ref().ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err("Plugin manager not initialized")
            })
        }
    }
}
