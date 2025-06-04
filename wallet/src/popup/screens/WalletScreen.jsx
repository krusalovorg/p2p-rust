import React, { useState } from 'react';

const WalletScreen = ({ peerId, apiEndpoint, onSettings, darkMode }) => {
  const [jsonRequest, setJsonRequest] = useState('');
  const [apiResponse, setApiResponse] = useState('');

  const handleCopyPeerId = () => {
    navigator.clipboard.writeText(peerId);
  };

  const handleSignAndSend = async () => {
    try {
      setApiResponse('Отправка запроса...\n' + jsonRequest);
      
      // Здесь будет реальная логика отправки запроса
      setTimeout(() => {
        setApiResponse('{\n  "status": "ok",\n  "result": "Пример ответа"\n}');
      }, 650);
    } catch (error) {
      setApiResponse(`Ошибка: ${error.message}`);
    }
  };

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-semibold">LP2LN Wallet</h1>
        <button
          onClick={onSettings}
          className="p-2 rounded-lg hover:bg-neutral-100 transition"
          title="Настройки"
        >
          <svg width="22" height="22" fill="none" viewBox="0 0 22 22">
            <circle cx="11" cy="11" r="9.5" stroke="#888" strokeWidth="1.5"/>
            <path stroke="#888" strokeWidth="1.5" strokeLinecap="round" d="M11 8v3.5M11 15h.01"/>
          </svg>
        </button>
      </div>

      <div className="mb-4">
        <div className="text-xs text-neutral-500 mb-1">Peer ID</div>
        <div className="flex items-center">
          <input
            type="text"
            value={peerId}
            readOnly
            className="flex-1 font-mono text-sm bg-neutral-50 border border-neutral-200 rounded-xl px-4 py-2 select-all mr-2 focus:bg-neutral-100 transition"
          />
          <button
            onClick={handleCopyPeerId}
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

      <div className="mb-4">
        <div className="text-xs text-neutral-500 mb-1">API endpoint</div>
        <input
          type="text"
          value={apiEndpoint}
          readOnly
          className="w-full px-4 py-2 bg-neutral-50 border border-neutral-200 rounded-xl text-sm focus:bg-neutral-100 transition font-mono"
        />
      </div>

      <div className="mb-4">
        <div className="text-xs text-neutral-500 mb-1">JSON-запрос</div>
        <textarea
          value={jsonRequest}
          onChange={(e) => setJsonRequest(e.target.value)}
          rows="6"
          className="w-full px-4 py-3 bg-neutral-50 border border-neutral-200 rounded-xl text-sm font-mono focus:bg-neutral-100 transition resize-none"
          placeholder='{\n  "action": "balance"\n}'
        />
      </div>

      <button
        onClick={handleSignAndSend}
        className="w-full py-3 px-4 bg-neutral-900 text-white hover:bg-neutral-800 transition rounded-xl text-base font-semibold mb-4"
      >
        Подписать и отправить
      </button>

      <div className="bg-neutral-50 border border-neutral-200 rounded-xl px-4 py-3 text-sm font-mono text-neutral-800 whitespace-pre overflow-x-auto min-h-[64px]">
        {apiResponse}
      </div>
    </div>
  );
};

export default WalletScreen; 