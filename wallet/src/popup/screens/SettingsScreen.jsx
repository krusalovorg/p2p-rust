import React from 'react';

const SettingsScreen = ({
  apiEndpoint,
  onApiEndpointChange,
  onBack,
  onDeleteKey,
  darkMode,
  onThemeToggle
}) => {
  return (
    <div>
      <div className="flex items-center mb-6">
        <button
          onClick={onBack}
          className="p-2 rounded-lg hover:bg-neutral-100 transition mr-2"
          title="Назад"
        >
          <svg width="22" height="22" fill="none" viewBox="0 0 22 22">
            <path d="M13.5 17l-5-6 5-6" stroke="#888" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round"/>
          </svg>
        </button>
        <h1 className="text-2xl font-semibold">Настройки</h1>
      </div>

      <div className="space-y-5">
        <div>
          <div className="text-xs text-neutral-500 mb-1">API endpoint</div>
          <input
            type="text"
            value={apiEndpoint}
            onChange={(e) => onApiEndpointChange(e.target.value)}
            className="w-full px-4 py-2 bg-neutral-50 border border-neutral-200 rounded-xl text-sm font-mono focus:bg-neutral-100 transition"
          />
        </div>

        <button
          onClick={() => alert('Ваш приватный ключ: 0x' + Math.random().toString(16).slice(2,18))}
          className="w-full py-3 px-4 bg-neutral-100 hover:bg-neutral-200 transition rounded-xl text-base font-medium"
        >
          Экспортировать приватный ключ
        </button>

        <button
          onClick={onDeleteKey}
          className="w-full py-3 px-4 bg-neutral-100 hover:bg-neutral-200 transition rounded-xl text-base font-medium text-red-600"
        >
          Удалить ключ
        </button>

        <div className="flex items-center justify-between pt-2">
          <span className="text-base text-neutral-700">Тема</span>
          <button
            onClick={onThemeToggle}
            className="px-4 py-2 bg-neutral-100 hover:bg-neutral-200 transition rounded-xl font-medium text-sm"
          >
            {darkMode ? 'Тёмная' : 'Светлая'}
          </button>
        </div>
      </div>
    </div>
  );
};

export default SettingsScreen; 