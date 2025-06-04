// Минимальная версия ethers.js для работы с ключами
class Wallet {
    constructor(privateKey) {
        this.privateKey = privateKey;
        this.publicKey = this.derivePublicKey(privateKey);
    }

    static createRandom() {
        const privateKey = this.generatePrivateKey();
        return new Wallet(privateKey);
    }

    static generatePrivateKey() {
        const array = new Uint8Array(32);
        crypto.getRandomValues(array);
        return Array.from(array)
            .map(b => b.toString(16).padStart(2, '0'))
            .join('');
    }

    derivePublicKey(privateKey) {
        // В реальном приложении здесь должна быть криптография
        // Для демонстрации просто хешируем приватный ключ
        const hash = this.keccak256(privateKey);
        return hash.slice(2, 42); // Берем первые 20 байт как публичный ключ
    }

    async signMessage(message) {
        const messageHash = this.keccak256(message);
        // В реальном приложении здесь должна быть подпись
        // Для демонстрации просто возвращаем хеш
        return messageHash;
    }

    keccak256(data) {
        // Простая реализация хеширования для демонстрации
        const encoder = new TextEncoder();
        const bytes = encoder.encode(data);
        const hashBuffer = crypto.subtle.digest('SHA-256', bytes);
        return '0x' + Array.from(new Uint8Array(hashBuffer))
            .map(b => b.toString(16).padStart(2, '0'))
            .join('');
    }
}

export { Wallet }; 