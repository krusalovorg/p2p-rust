import React, { useState } from 'react';

const IdentityScreen = ({ peerId, onGenerateKey, onImportKey, onContinue }) => {
  const [privateKey, setPrivateKey] = useState('');

  return (
    <div>
      <h1 className="text-2xl font-semibold mb-6 text-center">LP2LN Identity</h1>

      <button
        onClick={onGenerateKey}
        className="w-full mb-4 py-3 px-4 bg-neutral-100 hover:bg-neutral-200 transition rounded-xl text-base font-medium"
      >
        Создать новый ключ
      </button>

      <div className="mb-2">
        <input
          type="text"
          value={privateKey}
          onChange={(e) => setPrivateKey(e.target.value)}
          placeholder="Вставьте приватный ключ (hex)"
          className="w-full px-4 py-3 bg-neutral-50 border border-neutral-200 rounded-xl text-base focus:bg-neutral-100 transition"
          autoComplete="off"
        />
      </div>

      <button
        onClick={() => onImportKey(privateKey)}
        className="w-full mb-6 py-3 px-4 bg-neutral-100 hover:bg-neutral-200 transition rounded-xl text-base font-medium"
      >
        Импортировать
      </button>

      {peerId && (
        <div className="mb-8">
          <div className="text-xs text-neutral-500 mb-1">Peer ID</div>
          <div className="font-mono text-sm bg-neutral-50 border border-neutral-200 rounded-xl px-4 py-2 select-all">
            {peerId}
          </div>
        </div>
      )}

      <button
        onClick={onContinue}
        disabled={!peerId}
        className="w-full py-3 px-4 bg-neutral-900 text-white hover:bg-neutral-800 transition rounded-xl text-base font-semibold disabled:opacity-50"
      >
        Продолжить
      </button>
    </div>
  );
};

export default IdentityScreen; 