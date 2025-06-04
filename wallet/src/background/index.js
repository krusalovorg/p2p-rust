// Конфигурация API
const API_BASE_URL = 'http://127.0.0.1:8081';

// Обработка сообщений от popup
chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
    if (message.type === 'connect') {
        checkConnection();
    } else if (message.type === 'send_packet') {
        sendPacket(message.data);
    } else if (message.type === 'send_test_message') {
        sendTestMessage(message.text);
    }
});

// Функция проверки соединения с API
async function checkConnection() {
    try {
        const response = await fetch(`${API_BASE_URL}/api/info`);
        if (response.ok) {
            const data = await response.json();
            chrome.runtime.sendMessage({
                type: 'connection_status',
                connected: true,
                message: 'Подключено к P2P сети',
                nodeInfo: data
            });
        } else {
            throw new Error('Ошибка подключения к API');
        }
    } catch (error) {
        console.error('Ошибка проверки соединения:', error);
        chrome.runtime.sendMessage({
            type: 'connection_status',
            connected: false,
            message: `Ошибка подключения: ${error.message}`
        });
    }
}

// Функция отправки тестового сообщения
async function sendTestMessage(text) {
    const packet = {
        to: null, // Отправляем всем
        data: {
            Message: {
                text: text,
                nonce: null
            }
        },
        act: "peer_list",
        protocol: "TURN"
    };

    try {
        const response = await fetch(`${API_BASE_URL}/api/packet`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify(packet)
        });

        if (!response.ok) {
            throw new Error(`HTTP error! status: ${response.status}`);
        }

        const data = await response.json();
        chrome.runtime.sendMessage({
            type: 'response',
            data: data
        });
    } catch (error) {
        console.error('Ошибка отправки тестового сообщения:', error);
        chrome.runtime.sendMessage({
            type: 'connection_status',
            connected: false,
            message: `Ошибка отправки сообщения: ${error.message}`
        });
    }
}

// Функция отправки пакета
async function sendPacket(packet) {
    try {
        const response = await fetch(`${API_BASE_URL}/api/packet`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify(packet)
        });

        if (!response.ok) {
            throw new Error(`HTTP error! status: ${response.status}`);
        }

        const data = await response.json();
        chrome.runtime.sendMessage({
            type: 'response',
            data: data
        });
    } catch (error) {
        console.error('Ошибка отправки пакета:', error);
        chrome.runtime.sendMessage({
            type: 'connection_status',
            connected: false,
            message: `Ошибка отправки пакета: ${error.message}`
        });
    }
}

// Обработка сообщений от content script
chrome.runtime.onMessage.addListener((request, sender, sendResponse) => {
    if (request.type === 'SIGN_PACKET') {
        // Получаем приватный ключ из хранилища
        chrome.storage.local.get(['privateKey'], async (result) => {
            if (result.privateKey) {
                try {
                    const wallet = new ethers.Wallet(result.privateKey);
                    const messageHash = ethers.utils.keccak256(JSON.stringify(request.packet));
                    const signature = await wallet.signMessage(ethers.utils.arrayify(messageHash));
                    sendResponse({ success: true, signature });
                } catch (error) {
                    sendResponse({ success: false, error: error.message });
                }
            } else {
                sendResponse({ success: false, error: 'Ключ не найден' });
            }
        });
        return true; // Указываем, что ответ будет асинхронным
    }
}); 