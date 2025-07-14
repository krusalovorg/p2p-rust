import json
from typing import Optional, Dict, Any
from flask import Flask, jsonify, request, render_template
from threading import Thread
from p2p_rust import (
    sign_message, verify_signature, get_network_stats,
    get_peer_info, send_packet, get_storage_stats,
    get_private_key, get_public_key, get_peer_connections,
    get_token_info
)

app = Flask(__name__)

# Глобальная переменная для хранения последних пакетов
last_packets = []

def process_packet(packet: Dict[str, Any]) -> Optional[Dict[str, Any]]:
    """
    Обработка входящего пакета и сохранение его для веб-интерфейса
    """
    # Сохраняем пакет для отображения в веб-интерфейсе
    last_packets.append(packet)
    if len(last_packets) > 100:  # Ограничиваем историю
        last_packets.pop(0)
    
    # Если это сообщение, подписываем его
    if packet.get('act') == 'message' and 'data' in packet:
        if isinstance(packet['data'], dict) and 'text' in packet['data']:
            message = packet['data']['text']
            private_key = get_private_key()
            signature = sign_message(message, private_key)
            packet['signature'] = signature
    
    return packet

# Веб-маршруты
@app.route('/')
def index():
    """Главная страница"""
    return render_template('index.html')

@app.route('/api/stats')
def get_stats():
    """Получение статистики сети"""
    stats = json.loads(get_network_stats())
    return jsonify(stats)

@app.route('/api/peer/<peer_id>')
def get_peer(peer_id):
    """Получение информации о пире"""
    info = json.loads(get_peer_info(peer_id))
    return jsonify(info)

@app.route('/api/storage')
def get_storage():
    """Получение статистики хранилища"""
    stats = json.loads(get_storage_stats())
    return jsonify(stats)

@app.route('/api/packets')
def get_packets():
    """Получение последних пакетов"""
    return jsonify(last_packets)

@app.route('/api/connections')
def get_connections():
    """Получение информации о соединениях"""
    connections = json.loads(get_peer_connections())
    return jsonify(connections)

@app.route('/api/token/<token>')
def get_token(token):
    """Получение информации о токене"""
    info = json.loads(get_token_info(token))
    return jsonify(info)

@app.route('/api/keys')
def get_keys():
    """Получение информации о ключах"""
    return jsonify({
        "public_key": get_public_key(),
        "has_private_key": bool(get_private_key())
    })

@app.route('/api/send', methods=['POST'])
def send():
    """Отправка пакета в сеть"""
    packet = request.json
    
    # Если нужно подписать пакет
    if request.args.get('sign') == 'true':
        if 'data' in packet and 'text' in packet['data']:
            message = packet['data']['text']
            private_key = get_private_key()
            signature = sign_message(message, private_key)
            packet['signature'] = signature
    
    if send_packet(json.dumps(packet)):
        return jsonify({"status": "success"})
    return jsonify({"status": "error"}), 400

def start_web_server():
    """Запуск веб-сервера в отдельном потоке"""
    app.run(host='0.0.0.0', port=5000)

# Запускаем веб-сервер при загрузке плагина
web_thread = Thread(target=start_web_server, daemon=True)
web_thread.start()

if __name__ == "__main__":
    # Тестовый код
    test_packet = {
        "act": "message",
        "data": {
            "text": "Test message"
        }
    }
    result = process_packet(test_packet)
    print(json.dumps(result)) 