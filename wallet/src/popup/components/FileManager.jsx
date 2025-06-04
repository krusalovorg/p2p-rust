import React, { useState, useEffect } from 'react';
import { uploadFile, fetchFiles, getFileUrl, validateFile } from '../services/fileService';

const FileManager = () => {
    const [files, setFiles] = useState([]);
    const [selectedFile, setSelectedFile] = useState(null);
    const [isUploading, setIsUploading] = useState(false);
    const [error, setError] = useState('');
    const [success, setSuccess] = useState('');
    const [termsAccepted, setTermsAccepted] = useState(false);

    const loadFiles = async () => {
        try {
            const filesList = await fetchFiles();
            setFiles(filesList);
        } catch (err) {
            setError('Ошибка при загрузке файлов: ' + err.message);
        }
    };

    useEffect(() => {
        loadFiles();
    }, []);

    const handleFileSelect = (event) => {
        const file = event.target.files[0];
        if (file) {
            const validation = validateFile(file);
            if (!validation.isValid) {
                setError(validation.errors[0]);
                event.target.value = '';
                return;
            }
            setSelectedFile(file);
            setError('');
        }
    };

    const handleUpload = async () => {
        if (!selectedFile) {
            setError('Пожалуйста, выберите файл для загрузки.');
            return;
        }

        if (!termsAccepted) {
            setError('Пожалуйста, примите Пользовательское соглашение перед загрузкой.');
            return;
        }

        try {
            setIsUploading(true);
            setError('');
            setSuccess('');

            const result = await uploadFile(selectedFile);
            
            setSuccess(`Файл "${selectedFile.name}" успешно загружен!`);
            setSelectedFile(null);
            document.getElementById('file-input').value = '';
            setTermsAccepted(false);
            
            await loadFiles();
        } catch (err) {
            setError('Ошибка при загрузке файла: ' + err.message);
        } finally {
            setIsUploading(false);
        }
    };

    return (
        <div className="w-full max-w-md mx-auto">
            <div className="flex items-center justify-between mb-6">
                <h1 className="text-2xl font-semibold">Файловый менеджер</h1>
                <button className="p-2 rounded-lg hover:bg-neutral-100 transition" title="Настройки">
                    <svg width="22" height="22" fill="none" viewBox="0 0 22 22">
                        <circle cx="11" cy="11" r="9.5" stroke="#888" strokeWidth="1.5"/>
                        <path stroke="#888" strokeWidth="1.5" strokeLinecap="round" d="M11 8v3.5M11 15h.01"/>
                    </svg>
                </button>
            </div>

            {error && (
                <div className="mb-4 bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded-xl text-sm">
                    {error}
                </div>
            )}

            {success && (
                <div className="mb-4 bg-green-100 border border-green-400 text-green-700 px-4 py-3 rounded-xl text-sm">
                    {success}
                </div>
            )}

            <div className="space-y-4">
                {/* Выбор файла */}
                <div className="mb-4">
                    <div className="text-xs text-neutral-500 mb-1">Выберите файл</div>
                    <input
                        type="file"
                        id="file-input"
                        onChange={handleFileSelect}
                        className="hidden"
                    />
                    <label
                        htmlFor="file-input"
                        className="block w-full px-4 py-3 bg-neutral-50 border border-neutral-200 rounded-xl text-base cursor-pointer hover:bg-neutral-100 transition"
                    >
                        {selectedFile ? selectedFile.name : 'Выберите файл'}
                    </label>
                </div>

                {/* Соглашение */}
                <div className="mb-4">
                    <label className="flex items-center space-x-2">
                        <input
                            type="checkbox"
                            checked={termsAccepted}
                            onChange={(e) => setTermsAccepted(e.target.checked)}
                            className="rounded border-neutral-200"
                        />
                        <span className="text-sm text-neutral-700">
                            Я принимаю Пользовательское соглашение
                        </span>
                    </label>
                </div>

                {/* Кнопка загрузки */}
                <button
                    onClick={handleUpload}
                    disabled={isUploading || !selectedFile || !termsAccepted}
                    className="w-full py-3 px-4 bg-neutral-900 text-white hover:bg-neutral-800 transition rounded-xl text-base font-semibold disabled:opacity-50"
                >
                    {isUploading ? 'Загрузка...' : 'Загрузить файл'}
                </button>

                {/* Список файлов */}
                <div className="mt-8">
                    <h3 className="text-lg font-semibold mb-4">Все файлы</h3>
                    <div className="space-y-3">
                        {files.map((file) => (
                            <div key={file.file_hash} className="bg-neutral-50 border border-neutral-200 rounded-xl p-4">
                                <div className="flex justify-between items-start mb-2">
                                    <div>
                                        <h4 className="font-medium text-neutral-900">{file.filename}</h4>
                                        <p className="text-sm text-neutral-500 font-mono">
                                            {file.file_hash.substring(0, 12)}...
                                        </p>
                                    </div>
                                    <span className={`text-xs px-2 py-1 rounded ${
                                        file.public ? 'bg-green-100 text-green-700' : 'bg-yellow-100 text-yellow-700'
                                    }`}>
                                        {file.public ? 'Публичный' : 'Приватный'}
                                    </span>
                                </div>
                                <div className="flex justify-between items-center">
                                    <span className="text-sm text-neutral-500">
                                        {(file.size / 1024).toFixed(2)} KB
                                    </span>
                                    <button
                                        onClick={() => navigator.clipboard.writeText(getFileUrl(file.file_hash))}
                                        className="text-sm text-neutral-600 hover:text-neutral-900"
                                    >
                                        Копировать ссылку
                                    </button>
                                </div>
                            </div>
                        ))}
                    </div>
                </div>
            </div>
        </div>
    );
};

export default FileManager; 