import React, { useState, useEffect } from 'react';
import EC from 'elliptic';
import CryptoJS from 'crypto-js';

const KeyManager = () => {
    const [privateKey, setPrivateKey] = useState('');
    const [publicKey, setPublicKey] = useState('');
    const [isLoading, setIsLoading] = useState(false);
    const [error, setError] = useState('');

    const generateNewKeyPair = async () => {
        try {
            setIsLoading(true);
            setError('');

            // Инициализация эллиптической кривой secp256k1
            const ec = new EC.ec('secp256k1');
            
            // Генерация пары ключей
            const keyPair = ec.genKeyPair();
            
            // Получение приватного ключа в hex формате
            const privateKeyHex = keyPair.getPrivate('hex');
            
            // Получение публичного ключа в сжатом формате
            const publicKeyHex = keyPair.getPublic(true, 'hex');

            // Сохранение ключей в chrome.storage
            await chrome.storage.local.set({
                privateKey: privateKeyHex,
                publicKey: publicKeyHex
            });

            setPrivateKey(privateKeyHex);
            setPublicKey(publicKeyHex);
        } catch (err) {
            setError('Ошибка при генерации ключей: ' + err.message);
        } finally {
            setIsLoading(false);
        }
    };

    const loadExistingKeys = async () => {
        try {
            const data = await chrome.storage.local.get(['privateKey', 'publicKey']);
            if (data.privateKey && data.publicKey) {
                setPrivateKey(data.privateKey);
                setPublicKey(data.publicKey);
            }
        } catch (err) {
            setError('Ошибка при загрузке ключей: ' + err.message);
        }
    };

    useEffect(() => {
        loadExistingKeys();
    }, []);

    return (
        <div className="w-full max-w-md mx-auto">
            <div className="flex items-center justify-between mb-6">
                <h1 className="text-2xl font-semibold">Управление ключами</h1>
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

            <div className="space-y-4">
                {/* Peer ID с копированием */}
                <div className="mb-4">
                    <div className="text-xs text-neutral-500 mb-1">Peer ID</div>
                    <div className="flex items-center">
                        <input
                            type="text"
                            value={publicKey}
                            readOnly
                            className="flex-1 font-mono text-sm bg-neutral-50 border border-neutral-200 rounded-xl px-4 py-2 select-all mr-2 focus:bg-neutral-100 transition"
                        />
                        <button
                            onClick={() => navigator.clipboard.writeText(publicKey)}
                            className="p-2 rounded-lg hover:bg-neutral-100 transition"
                            title="Скопировать"
                        >
                            <svg width="18" height="18" fill="none" viewBox="0 0 18 18">
                                <rect x="4" y="4" width="10" height="10" rx="2" stroke="#888" strokeWidth="1.2"/>
                                <rect x="7" y="2" width="9" height="9" rx="2" stroke="#bbb" strokeWidth="1.2"/>
                            </svg>
                        </button>
                    </div>
                </div>

                {/* Приватный ключ */}
                <div className="mb-4">
                    <div className="text-xs text-neutral-500 mb-1">Приватный ключ</div>
                    <div className="flex items-center">
                        <input
                            type="password"
                            value={privateKey}
                            readOnly
                            className="flex-1 font-mono text-sm bg-neutral-50 border border-neutral-200 rounded-xl px-4 py-2 select-all mr-2 focus:bg-neutral-100 transition"
                        />
                        <button
                            onClick={() => navigator.clipboard.writeText(privateKey)}
                            className="p-2 rounded-lg hover:bg-neutral-100 transition"
                            title="Скопировать"
                        >
                            <svg width="18" height="18" fill="none" viewBox="0 0 18 18">
                                <rect x="4" y="4" width="10" height="10" rx="2" stroke="#888" strokeWidth="1.2"/>
                                <rect x="7" y="2" width="9" height="9" rx="2" stroke="#bbb" strokeWidth="1.2"/>
                            </svg>
                        </button>
                    </div>
                </div>

                {/* Кнопки действий */}
                <div className="space-y-2">
                    <button
                        onClick={generateNewKeyPair}
                        disabled={isLoading}
                        className="w-full py-3 px-4 bg-neutral-100 hover:bg-neutral-200 transition rounded-xl text-base font-medium"
                    >
                        {isLoading ? 'Генерация...' : 'Создать новый ключ'}
                    </button>
                    <button
                        onClick={() => {
                            if(confirm('Удалить приватный ключ?')) {
                                chrome.storage.local.remove(['privateKey', 'publicKey']);
                                setPrivateKey('');
                                setPublicKey('');
                            }
                        }}
                        className="w-full py-3 px-4 bg-neutral-100 hover:bg-neutral-200 transition rounded-xl text-base font-medium text-red-600"
                    >
                        Удалить ключ
                    </button>
                </div>
            </div>
        </div>
    );
};

export default KeyManager; 