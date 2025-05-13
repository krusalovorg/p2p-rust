#[cfg(test)]
mod tests {
    use p2p_server::db::P2PDatabase;
    use tempfile::tempdir;

    #[test]
    fn test_database_creation() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().to_str().unwrap();

        let db = P2PDatabase::new(db_path).unwrap();
        assert_eq!(db.path, db_path);

        // Проверяем, что файл базы данных был создан
        let db_file = temp_dir.path().join("db");
        assert!(db_file.exists());
    }

    #[test]
    fn test_database_clone() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().to_str().unwrap();

        let db1 = P2PDatabase::new(db_path).unwrap();
        let db2 = db1.clone();

        assert_eq!(db1.path, db2.path);
    }
} 