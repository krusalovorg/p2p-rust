import React, { useState } from 'react';
import { HashRouter as Router, Routes, Route, useNavigate, useLocation } from 'react-router-dom';
import FileManager from './components/FileManager';
import KeyManager from './components/KeyManager';
import ContractManager from './components/ContractManager';
import FileExplorer from './components/FileExplorer';
import FileViewer from './components/FileViewer';

const AppContent = () => {
    const [activeTab, setActiveTab] = useState('files');
    const navigate = useNavigate();
    const location = useLocation();

    const handleTabChange = (tab) => {
        setActiveTab(tab);
        if (tab === 'files') {
            navigate('/');
        }
    };

    return (
        <div className="min-h-screen bg-neutral-50">
            <div className="max-w-4xl mx-auto">
                <div className="sticky z-10 top-0 bg-white border-b border-neutral-200">
                    <div className="max-w-4xl mx-auto px-4 py-3">
                        <div className="flex items-center justify-between">
                            <div className="flex items-center space-x-3">
                                <div className="w-8 h-8 bg-black rounded-lg flex items-center justify-center">
                                    <svg width="20" height="20" fill="none" viewBox="0 0 20 20">
                                        <path d="M10 2.5L2.5 5.833V14.167L10 17.5L17.5 14.167V5.833L10 2.5Z" stroke="white" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
                                        <path d="M10 10L10 17.5M10 10L17.5 14.167M10 10L2.5 14.167" stroke="white" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
                                    </svg>
                                </div>
                                <div>
                                    <h1 className="text-lg font-semibold">LP2LN Wallet</h1>
                                    <p className="text-xs text-neutral-500">P2P файловый менеджер</p>
                                </div>
                            </div>
                            <div className="flex items-center space-x-2">
                                <button
                                    className="p-2 rounded-lg hover:bg-neutral-100 transition"
                                    title="Настройки"
                                >
                                    <svg width="20" height="20" fill="none" viewBox="0 0 20 20">
                                        <path d="M10 13.333a3.333 3.333 0 100-6.666 3.333 3.333 0 000 6.666z" stroke="#888" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
                                        <path d="M16.667 10c0-.684-.055-1.35-.158-1.994l2.5-1.667-1.667-2.5-2.5.833a6.667 6.667 0 00-1.175-1.175l-.833-2.5-2.5 1.667A6.667 6.667 0 0010 3.333c-.684 0-1.35.055-1.994.158l-1.667-2.5-2.5.833.833 2.5a6.667 6.667 0 00-1.175 1.175l-2.5-.833-1.667 2.5 2.5 1.667a6.667 6.667 0 000 3.988l-2.5 1.667 1.667 2.5 2.5-.833c.342.442.742.842 1.175 1.175l.833 2.5 2.5-1.667c.644.103 1.31.158 1.994.158.684 0 1.35-.055 1.994-.158l1.667 2.5 2.5-.833-.833-2.5c.433-.333.833-.733 1.175-1.175l2.5.833 1.667-2.5-2.5-1.667c.103-.644.158-1.31.158-1.994z" stroke="#888" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
                                    </svg>
                                </button>
                                <button
                                    className="p-2 rounded-lg hover:bg-neutral-100 transition"
                                    title="Помощь"
                                >
                                    <svg width="20" height="20" fill="none" viewBox="0 0 20 20">
                                        <circle cx="10" cy="10" r="7.5" stroke="#888" strokeWidth="1.5"/>
                                        <path d="M10 14.167v-.834c0-.92.747-1.666 1.667-1.666.92 0 1.666.746 1.666 1.666v.834M10 7.5v.833" stroke="#888" strokeWidth="1.5" strokeLinecap="round"/>
                                    </svg>
                                </button>
                            </div>
                        </div>
                    </div>
                </div>

                <div className="p-4">
                    <div className="flex space-x-2 mb-4">
                        <button
                            onClick={() => handleTabChange('files')}
                            className={`flex-1 py-2 px-4 rounded-xl transition ${
                                activeTab === 'files' ? 'bg-black text-white' : 'bg-white hover:bg-neutral-100'
                            }`}
                        >
                            Файлы
                        </button>
                        <button
                            onClick={() => handleTabChange('explorer')}
                            className={`flex-1 py-2 px-4 rounded-xl transition ${
                                activeTab === 'explorer' ? 'bg-black text-white' : 'bg-white hover:bg-neutral-100'
                            }`}
                        >
                            Обзор
                        </button>
                        <button
                            onClick={() => handleTabChange('keys')}
                            className={`flex-1 py-2 px-4 rounded-xl transition ${
                                activeTab === 'keys' ? 'bg-black text-white' : 'bg-white hover:bg-neutral-100'
                            }`}
                        >
                            Ключи
                        </button>
                        <button
                            onClick={() => handleTabChange('contracts')}
                            className={`flex-1 py-2 px-4 rounded-xl transition ${
                                activeTab === 'contracts' ? 'bg-black text-white' : 'bg-white hover:bg-neutral-100'
                            }`}
                        >
                            Контракты
                        </button>
                    </div>

                    <Routes>
                        <Route path="/" element={
                            activeTab === 'files' ? <FileManager /> :
                            activeTab === 'explorer' ? <FileExplorer /> :
                            activeTab === 'keys' ? <KeyManager /> :
                            activeTab === 'contracts' ? <ContractManager /> : null
                        } />
                        <Route path="/file/:hash" element={<FileViewer />} />
                    </Routes>
                </div>
            </div>
        </div>
    );
};

const App = () => {
    return (
        <Router>
            <AppContent />
        </Router>
    );
};

export default App; 