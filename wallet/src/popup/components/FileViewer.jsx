import React, { useState, useEffect } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { getFile, getAllFiles } from '../services/api';

const FileViewer = () => {
    const { hash } = useParams();
    const navigate = useNavigate();
    const [file, setFile] = useState(null);
    const [preview, setPreview] = useState(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState(null);

    useEffect(() => {
        const loadFile = async () => {
            try {
                setLoading(true);
                setError(null);
                
                // Получаем список всех файлов
                const files = await getAllFiles();
                const fileData = files.find(f => f.file_hash === hash);
                
                if (!fileData) {
                    throw new Error('Файл не найден');
                }
                
                setFile(fileData);
                
                // Загружаем содержимое файла
                const blob = await getFile(hash);
                const url = URL.createObjectURL(blob);
                setPreview(url);
            } catch (err) {
                setError(err.message);
            } finally {
                setLoading(false);
            }
        };

        loadFile();

        return () => {
            if (preview) {
                URL.revokeObjectURL(preview);
            }
        };
    }, [hash]);

    const handleShare = async () => {
        try {
            const blob = await getFile(file.file_hash);
            const fileToShare = new File([blob], getFileName(file), { type: file.mime });
            
            if (navigator.share) {
                await navigator.share({
                    title: getFileName(file),
                    files: [fileToShare]
                });
            } else {
                await navigator.clipboard.writeText(file.file_hash);
                setSuccess('Хеш файла скопирован в буфер обмена');
            }
        } catch (err) {
            setError(err.message);
        }
    };

    const getFileName = (file) => {
        if (!file) return 'Без имени';
        const ext = getFileExtension(file.mime);
        const hash = formatFileHash(file.file_hash);
        return `${hash}${ext ? `.${ext}` : ''}`;
    };

    const getFileExtension = (mime) => {
        if (!mime) return '';
        
        const mimeToExt = {
            'text/html': 'html',
            'text/css': 'css',
            'text/javascript': 'js',
            'application/wasm': 'wasm',
            'image/png': 'png',
            'image/jpeg': 'jpg',
            'image/gif': 'gif',
            'image/webp': 'webp',
            'application/pdf': 'pdf'
        };
        
        return mimeToExt[mime] || mime.split('/')[1] || '';
    };

    const formatFileHash = (hash) => {
        if (!hash) return '';
        return `${hash.slice(0, 8)}...${hash.slice(-8)}`;
    };

    const renderPreview = () => {
        if (loading) {
            return (
                <div className="flex items-center justify-center h-[60vh]">
                    <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-neutral-400"></div>
                </div>
            );
        }

        if (error) {
            return (
                <div className="text-center py-8 text-red-500">
                    {error}
                </div>
            );
        }

        if (!preview || !file) return null;

        const mime = file.mime || '';
        
        if (mime.startsWith('image/')) {
            return <img src={preview} alt={getFileName(file)} className="max-w-full h-auto" />;
        }
        
        if (mime.startsWith('text/')) {
            return (
                <div className="bg-neutral-50 rounded-lg p-4 font-mono text-sm overflow-auto max-h-[60vh]">
                    <pre>{preview}</pre>
                </div>
            );
        }
        
        if (mime === 'application/pdf') {
            return (
                <iframe
                    src={preview}
                    className="w-full h-[60vh]"
                    title={getFileName(file)}
                />
            );
        }

        return (
            <div className="text-center py-8 text-neutral-500">
                Предпросмотр недоступен для этого типа файла
            </div>
        );
    };

    if (loading) {
        return (
            <div className="min-h-screen bg-neutral-50 flex items-center justify-center">
                <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-neutral-400"></div>
            </div>
        );
    }

    if (error) {
        return (
            <div className="min-h-screen bg-neutral-50 flex items-center justify-center">
                <div className="text-center">
                    <div className="text-red-500 mb-4">{error}</div>
                    <button
                        onClick={() => navigate('/')}
                        className="px-4 py-2 bg-black text-white rounded-xl hover:bg-neutral-800 transition"
                    >
                        Вернуться назад
                    </button>
                </div>
            </div>
        );
    }

    if (!file) return null;

    return (
        <div className="min-h-screen bg-neutral-50">
            <div className="max-w-4xl mx-auto p-4">
                <div className="bg-white rounded-xl shadow-sm">
                    <div className="flex items-center justify-between p-4 border-b border-neutral-200">
                        <div className="flex items-center space-x-4">
                            <button
                                onClick={() => navigate('/')}
                                className="p-2 rounded-lg hover:bg-neutral-100 transition"
                            >
                                <svg width="20" height="20" fill="none" viewBox="0 0 20 20">
                                    <path d="M12.5 15L7.5 10L12.5 5" stroke="#888" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
                                </svg>
                            </button>
                            <div className="text-lg font-medium">{getFileName(file)}</div>
                        </div>
                        <div className="flex items-center space-x-2">
                            <button
                                onClick={handleShare}
                                className="p-2 rounded-lg hover:bg-neutral-100 transition"
                                title="Поделиться"
                            >
                                <svg width="20" height="20" fill="none" viewBox="0 0 20 20">
                                    <path d="M15 6.667a2 2 0 100-4 2 2 0 000 4zM5 13.333a2 2 0 100-4 2 2 0 000 4zM15 17.333a2 2 0 100-4 2 2 0 000 4zM7.159 11.84l5.682 3.32M7.159 8.827l5.682-3.32" stroke="#888" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
                                </svg>
                            </button>
                        </div>
                    </div>

                    <div className="p-4">
                        <div className="flex items-center space-x-2 mb-4">
                            {file.public ? (
                                <span className="text-xs text-green-600">Публичный</span>
                            ) : (
                                <span className="text-xs text-red-600">Приватный</span>
                            )}
                            {file.encrypted && (
                                <span className="text-xs text-blue-600">Зашифрован</span>
                            )}
                            {file.compressed && (
                                <span className="text-xs text-purple-600">Сжат</span>
                            )}
                            {file.is_contract && (
                                <span className="text-xs text-orange-600">Контракт</span>
                            )}
                        </div>

                        <div className="bg-neutral-50 rounded-xl p-4 mb-4">
                            <div className="grid grid-cols-2 gap-4">
                                <div>
                                    <div className="text-sm text-neutral-500 mb-1">Хеш файла</div>
                                    <div className="font-mono text-sm break-all">{file.file_hash}</div>
                                </div>
                                <div>
                                    <div className="text-sm text-neutral-500 mb-1">MIME-тип</div>
                                    <div className="font-mono text-sm">{file.mime || 'Не указан'}</div>
                                </div>
                                {file.filename && (
                                    <div>
                                        <div className="text-sm text-neutral-500 mb-1">Имя файла</div>
                                        <div className="font-mono text-sm">{file.filename}</div>
                                    </div>
                                )}
                                {file.size && (
                                    <div>
                                        <div className="text-sm text-neutral-500 mb-1">Размер</div>
                                        <div className="font-mono text-sm">{formatFileSize(file.size)}</div>
                                    </div>
                                )}
                            </div>
                        </div>

                        <div className="bg-white rounded-xl border border-neutral-200 p-4">
                            {renderPreview()}
                        </div>
                    </div>
                </div>
            </div>
        </div>
    );
};

const formatFileSize = (bytes) => {
    if (!bytes) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
};

export default FileViewer; 