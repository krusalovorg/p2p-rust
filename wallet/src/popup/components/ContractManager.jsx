import React, { useState } from 'react';
import { callContract } from '../services/api';

const ContractManager = () => {
    const [contractHash, setContractHash] = useState('');
    const [functionName, setFunctionName] = useState('');
    const [payload, setPayload] = useState('');
    const [result, setResult] = useState(null);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState(null);

    const handleCallContract = async () => {
        if (!contractHash || !functionName) {
            setError('Пожалуйста, заполните все обязательные поля');
            return;
        }

        try {
            setLoading(true);
            setError(null);
            setResult(null);
            const response = await callContract(contractHash, functionName, payload);
            setResult(response.result);
        } catch (err) {
            setError(err.message);
        } finally {
            setLoading(false);
        }
    };

    return (
        <div>
            {error && (
                <div className="bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded-xl mb-4 text-sm">
                    {error}
                </div>
            )}

            <div className="mb-4">
                <div className="text-xs text-neutral-500 mb-1">Хеш контракта</div>
                <input
                    type="text"
                    value={contractHash}
                    onChange={(e) => setContractHash(e.target.value)}
                    placeholder="Введите хеш контракта"
                    className="w-full px-4 py-3 bg-white border border-neutral-200 rounded-xl text-sm font-mono focus:bg-neutral-50 transition"
                />
            </div>

            <div className="mb-4">
                <div className="text-xs text-neutral-500 mb-1">Имя функции</div>
                <input
                    type="text"
                    value={functionName}
                    onChange={(e) => setFunctionName(e.target.value)}
                    placeholder="Введите имя функции"
                    className="w-full px-4 py-3 bg-white border border-neutral-200 rounded-xl text-sm font-mono focus:bg-neutral-50 transition"
                />
            </div>

            <div className="mb-6">
                <div className="text-xs text-neutral-500 mb-1">Полезная нагрузка (опционально)</div>
                <textarea
                    value={payload}
                    onChange={(e) => setPayload(e.target.value)}
                    placeholder="Введите JSON-объект"
                    className="w-full px-4 py-3 bg-white border border-neutral-200 rounded-xl text-sm font-mono focus:bg-neutral-50 transition resize-none h-32"
                />
            </div>

            <button
                onClick={handleCallContract}
                disabled={loading}
                className="w-full py-3 px-4 bg-neutral-900 text-white hover:bg-neutral-800 transition rounded-xl text-base font-semibold mb-4"
            >
                {loading ? 'Вызов...' : 'Подписать и отправить'}
            </button>

            {result && (
                <div>
                    <div className="text-xs text-neutral-500 mb-1">Ответ API</div>
                    <div className="bg-white border border-neutral-200 rounded-xl px-4 py-3 text-sm font-mono text-neutral-800 whitespace-pre overflow-x-auto min-h-[64px]">
                        {JSON.stringify(result, null, 2)}
                    </div>
                </div>
            )}
        </div>
    );
};

export default ContractManager; 