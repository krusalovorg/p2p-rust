// Обработка сообщений от веб-страницы
window.addEventListener('message', async (event) => {
    if (event.data.type === 'SIGN_PACKET') {
        try {
            const response = await chrome.runtime.sendMessage({
                type: 'SIGN_PACKET',
                packet: event.data.packet
            });
            
            // Отправляем ответ обратно на веб-страницу
            window.postMessage({
                type: 'SIGN_PACKET_RESPONSE',
                success: response.success,
                signature: response.signature,
                error: response.error
            }, '*');
        } catch (error) {
            window.postMessage({
                type: 'SIGN_PACKET_RESPONSE',
                success: false,
                error: error.message
            }, '*');
        }
    }
}); 