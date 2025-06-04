const API_BASE = 'http://lp2ln.tech:8081/api';

export const uploadFile = async (file, options = {}) => {
    const {
        isPublic = true,
        isEncrypted = false,
        isCompressed = false,
        autoDecompress = false,
        token = ''
    } = options;

    const formData = new FormData();
    formData.append('file', file);
    formData.append('public', isPublic.toString());
    formData.append('encrypted', isEncrypted.toString());
    formData.append('compressed', isCompressed.toString());
    formData.append('auto_decompress', autoDecompress.toString());
    formData.append('token', token);

    try {
        const response = await fetch(`${API_BASE}/upload`, {
            method: 'POST',
            body: formData
        });

        if (!response.ok) {
            const errorData = await response.json().catch(() => ({ message: "Ошибка сервера" }));
            throw new Error(errorData.message || `HTTP error! status: ${response.status}`);
        }

        return await response.json();
    } catch (error) {
        console.error("[FileService] Upload error:", error);
        throw error;
    }
};

export const fetchFiles = async () => {
    try {
        const response = await fetch(`${API_BASE}/files`);
        if (!response.ok) {
            throw new Error(`HTTP error! status: ${response.status}`);
        }
        return await response.json();
    } catch (error) {
        console.error("[FileService] Fetch files error:", error);
        throw error;
    }
};

export const getFileUrl = (fileHash) => {
    return `${window.location.protocol}//${window.location.hostname}:80/${fileHash}`;
};

export const validateFile = (file) => {
    const maxSize = 10 * 1024 * 1024; // 10 MB
    const errors = [];

    if (!file) {
        errors.push('Пожалуйста, выберите файл для загрузки.');
    } else {
        if (file.size > maxSize) {
            errors.push('Размер файла превышает 10 МБ. Пожалуйста, выберите файл меньшего размера.');
        }
    }

    return {
        isValid: errors.length === 0,
        errors
    };
}; 