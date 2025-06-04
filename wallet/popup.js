// Импортируем локальную версию ethers
import { Wallet } from './lib/ethers.js';

class LP2LNWallet {
    constructor() {
        this.privateKey = null;
        this.publicKey = null;
        this.connected = false;
        this.port = null;
        
        // Инициализация UI элементов
        this.initializeUI();
    }

    initializeUI() {
        // Кнопки и поля ввода
        this.generateKeyBtn = document.getElementById('generateKey');
        this.importKeyInput = document.getElementById('importKey');
        this.importKeyBtn = document.getElementById('importKeyBtn');
        this.publicKeyDiv = document.getElementById('publicKey');
        this.connectBtn = document.getElementById('connect');
        this.connectionStatusDiv = document.getElementById('connectionStatus');
        this.packetDataTextarea = document.getElementById('packetData');
        this.sendPacketBtn = document.getElementById('sendPacket');
        this.responseDiv = document.getElementById('response');
        this.testMessageInput = document.getElementById('testMessage');
        this.sendTestMessageBtn = document.getElementById('sendTestMessage');

        // Привязка обработчиков событий
        this.generateKeyBtn.addEventListener('click', () => this.generateNewKey());
        this.importKeyBtn.addEventListener('click', () => this.importKey());
        this.connectBtn.addEventListener('click', () => this.connectToNetwork());
        this.sendPacketBtn.addEventListener('click', () => this.sendPacket());
        this.sendTestMessageBtn.addEventListener('click', () => this.sendTestMessage());

        // Загрузка сохраненных данных
        this.loadSavedData();

        // Слушаем сообщения от background script
        chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
            if (message.type === 'connection_status') {
                this.connected = message.connected;
                this.showConnectionStatus(
                    message.connected ? 'success' : 'error',
                    message.message
                );
            } else if (message.type === 'response') {
                this.showResponse('success', `Ответ от P2P сети: ${JSON.stringify(message.data, null, 2)}`);
            }
        });
    }

    async loadSavedData() {
        const data = await chrome.storage.local.get(['privateKey', 'publicKey', 'signalServer']);
        if (data.privateKey) {
            this.privateKey = data.privateKey;
            this.publicKey = data.publicKey;
            this.updatePublicKeyDisplay();
        }
        if (data.signalServer) {
            this.signalServerInput.value = data.signalServer;
        }
    }

    async generateNewKey() {
        try {
            const wallet = Wallet.createRandom();
            this.privateKey = wallet.privateKey;
            this.publicKey = wallet.publicKey;
            
            // Сохраняем ключи
            await chrome.storage.local.set({
                privateKey: this.privateKey,
                publicKey: this.publicKey
            });

            this.updatePublicKeyDisplay();
            this.showStatus('success', 'Новый ключ успешно сгенерирован');
        } catch (error) {
            this.showStatus('error', `Ошибка при генерации ключа: ${error.message}`);
        }
    }

    async importKey() {
        try {
            const privateKey = this.importKeyInput.value.trim();
            if (!privateKey) {
                throw new Error('Введите приватный ключ');
            }

            const wallet = new Wallet(privateKey);
            this.privateKey = wallet.privateKey;
            this.publicKey = wallet.publicKey;

            // Сохраняем ключи
            await chrome.storage.local.set({
                privateKey: this.privateKey,
                publicKey: this.publicKey
            });

            this.updatePublicKeyDisplay();
            this.showStatus('success', 'Ключ успешно импортирован');
        } catch (error) {
            this.showStatus('error', `Ошибка при импорте ключа: ${error.message}`);
        }
    }

    updatePublicKeyDisplay() {
        if (this.publicKey) {
            this.publicKeyDiv.textContent = `Публичный ключ: ${this.publicKey.slice(0, 10)}...${this.publicKey.slice(-8)}`;
            this.publicKeyDiv.className = 'status success';
        }
    }

    async connectToNetwork() {
        try {
            // Отправляем сообщение в background script для установки TCP соединения
            chrome.runtime.sendMessage({
                type: 'connect',
                host: 'lp2ln.space',
                port: 3031
            });
        } catch (error) {
            this.connected = false;
            this.showConnectionStatus('error', `Ошибка подключения: ${error.message}`);
        }
    }

    async sendPacket() {
        if (!this.connected) {
            this.showResponse('error', 'Сначала подключитесь к сети');
            return;
        }

        try {
            const packetData = this.packetDataTextarea.value.trim();
            if (!packetData) {
                throw new Error('Введите данные пакета');
            }

            const packet = JSON.parse(packetData);
            
            // Подписываем пакет
            const signature = await this.signPacket(packet);
            packet.signature = signature;

            // Отправляем пакет через background script
            chrome.runtime.sendMessage({
                type: 'send_packet',
                data: packet
            });
        } catch (error) {
            this.showResponse('error', `Ошибка: ${error.message}`);
        }
    }
    
    updatePublicKeyDisplay() {
        if (this.publicKey) {
            // Преобразуем публичный ключ в hex формат
            const hexKey = this.publicKey.slice(2); // Убираем '0x' префикс
            // Разбиваем на группы по 4 символа для лучшей читаемости
            const formattedKey = hexKey.match(/.{1,4}/g).join(' ');
            this.publicKeyDiv.textContent = `Публичный ключ: ${formattedKey}`;
            this.publicKeyDiv.className = 'status success';
        }
    } 

    async signPacket(packet) {
        if (!this.privateKey) {
            throw new Error('Ключ не найден');
        }

        const wallet = new Wallet(this.privateKey);
        const messageHash = ethers.utils.keccak256(JSON.stringify(packet));
        return await wallet.signMessage(ethers.utils.arrayify(messageHash));
    }

    showStatus(type, message) {
        this.publicKeyDiv.textContent = message;
        this.publicKeyDiv.className = `status ${type}`;
    }

    showConnectionStatus(type, message) {
        this.connectionStatusDiv.textContent = message;
        this.connectionStatusDiv.className = `status ${type}`;
    }

    showResponse(type, message) {
        this.responseDiv.textContent = message;
        this.responseDiv.className = `status ${type}`;
    }

    async sendTestMessage() {
        if (!this.connected) {
            this.showResponse('error', 'Сначала подключитесь к сети');
            return;
        }

        try {
            const message = this.testMessageInput.value.trim();
            if (!message) {
                throw new Error('Введите текст сообщения');
            }

            // Отправляем тестовое сообщение через background script
            chrome.runtime.sendMessage({
                type: 'send_test_message',
                text: message
            });
        } catch (error) {
            this.showResponse('error', `Ошибка: ${error.message}`);
        }
    }
}

// Инициализация при загрузке страницы
document.addEventListener('DOMContentLoaded', () => {
    window.wallet = new LP2LNWallet();
}); 