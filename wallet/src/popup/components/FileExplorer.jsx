import React, { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { getAllFiles } from '../services/api';
import FileViewer from './FileViewer';

const FileExplorer = () => {
    const navigate = useNavigate();
    const [files, setFiles] = useState([]);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState(null);
    const [selectedFile, setSelectedFile] = useState(null);
    const [view, setView] = useState('grid'); // 'grid' или 'list'
    const [filter, setFilter] = useState('all'); // 'all', 'public', 'private', 'encrypted', 'compressed'

    const fetchFiles = async () => {
        try {
            setLoading(true);
            setError(null);
            const filesList = await getAllFiles();
            setFiles(filesList);
        } catch (err) {
            setError(err.message);
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        fetchFiles();
    }, []);

    const getFileIcon = (file) => {
        if (!file) return '📁';
        
        const mime = file.mime || '';
        if (mime.startsWith('image/')) return '🖼️';
        if (mime === 'application/pdf') return '📄';
        if (mime === 'text/html') return '🌐';
        if (mime === 'text/css') return '🎨';
        if (mime === 'text/javascript') return '📜';
        if (mime === 'application/wasm') return '⚡';
        if (mime.startsWith('application/')) return '📦';
        return '📁';
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

    const getFileName = (file) => {
        if (!file) return 'Без имени';
        const ext = getFileExtension(file.mime);
        const hash = formatFileHash(file.file_hash);
        return `${hash}${ext ? `.${ext}` : ''}`;
    };

    const filteredFiles = files.filter(file => {
        if (!file) return false;
        
        switch (filter) {
            case 'public':
                return file.public;
            case 'private':
                return !file.public;
            case 'encrypted':
                return file.encrypted;
            case 'compressed':
                return file.compressed;
            default:
                return true;
        }
    });

    return (
        <div>
            {error && (
                <div className="bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded-xl mb-4 text-sm">
                    {error}
                </div>
            )}

            <div className="flex items-center justify-between mb-4">
                <div className="flex space-x-2">
                    <button
                        onClick={() => setView('grid')}
                        className={`p-2 rounded-lg transition ${view === 'grid' ? 'bg-neutral-200' : 'hover:bg-neutral-100'}`}
                        title="Сетка"
                    >
                        <svg width="20" height="20" fill="none" viewBox="0 0 20 20">
                            <rect x="3" y="3" width="6" height="6" rx="1" stroke="#888" strokeWidth="1.5"/>
                            <rect x="11" y="3" width="6" height="6" rx="1" stroke="#888" strokeWidth="1.5"/>
                            <rect x="3" y="11" width="6" height="6" rx="1" stroke="#888" strokeWidth="1.5"/>
                            <rect x="11" y="11" width="6" height="6" rx="1" stroke="#888" strokeWidth="1.5"/>
                        </svg>
                    </button>
                    <button
                        onClick={() => setView('list')}
                        className={`p-2 rounded-lg transition ${view === 'list' ? 'bg-neutral-200' : 'hover:bg-neutral-100'}`}
                        title="Список"
                    >
                        <svg width="20" height="20" fill="none" viewBox="0 0 20 20">
                            <path d="M3 5h14M3 10h14M3 15h14" stroke="#888" strokeWidth="1.5" strokeLinecap="round"/>
                        </svg>
                    </button>
                </div>

                <select
                    value={filter}
                    onChange={(e) => setFilter(e.target.value)}
                    className="px-3 py-2 bg-white border border-neutral-200 rounded-xl text-sm focus:outline-none focus:border-neutral-400"
                >
                    <option value="all">Все файлы</option>
                    <option value="public">Публичные</option>
                    <option value="private">Приватные</option>
                    <option value="encrypted">Зашифрованные</option>
                    <option value="compressed">Сжатые</option>
                </select>
            </div>

            {loading ? (
                <div className="text-center py-8 text-neutral-500">Загрузка...</div>
            ) : filteredFiles.length === 0 ? (
                <div className="text-center py-8 text-neutral-500">Нет доступных файлов</div>
            ) : view === 'grid' ? (
                <div className="grid grid-cols-2 gap-4">
                    {filteredFiles.map((file) => (
                        <div
                            key={file.file_hash}
                            className="bg-white border border-neutral-200 rounded-xl p-4 hover:border-neutral-400 transition"
                        >
                            <div className="text-2xl mb-2">{getFileIcon(file)}</div>
                            <div className="text-sm font-medium truncate">{getFileName(file)}</div>
                            <div className="text-xs text-neutral-500 font-mono truncate mt-1">
                                {file.file_hash}
                            </div>
                            <div className="flex items-center space-x-2 mt-2">
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
                            <div className="flex items-center space-x-2 mt-3">
                                <button
                                    onClick={() => navigate(`/file/${file.file_hash}`)}
                                    className="flex-1 py-2 px-3 bg-neutral-100 hover:bg-neutral-200 transition rounded-lg text-sm font-medium"
                                >
                                    Открыть
                                </button>
                            </div>
                        </div>
                    ))}
                </div>
            ) : (
                <div className="space-y-2">
                    {filteredFiles.map((file) => (
                        <div
                            key={file.file_hash}
                            className="bg-white border border-neutral-200 rounded-xl p-4 hover:border-neutral-400 transition"
                        >
                            <div className="flex items-center">
                                <div className="text-xl mr-3">{getFileIcon(file)}</div>
                                <div className="flex-1">
                                    <div className="text-sm font-medium">{getFileName(file)}</div>
                                    <div className="text-xs text-neutral-500 font-mono mt-1">
                                        {file.file_hash}
                                    </div>
                                </div>
                                <div className="flex items-center space-x-2">
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
                            </div>
                            <div className="flex items-center space-x-2 mt-3">
                                <button
                                    onClick={() => navigate(`/file/${file.file_hash}`)}
                                    className="flex-1 py-2 px-3 bg-neutral-100 hover:bg-neutral-200 transition rounded-lg text-sm font-medium"
                                >
                                    Открыть
                                </button>
                            </div>
                        </div>
                    ))}
                </div>
            )}

            {selectedFile && (
                <FileViewer
                    file={selectedFile}
                    onClose={() => setSelectedFile(null)}
                />
            )}
        </div>
    );
};

export default FileExplorer; 