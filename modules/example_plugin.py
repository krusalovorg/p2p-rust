import sys
from typing import Optional, Dict, Any

print("Available modules:", list(sys.modules.keys()))
print("Current directory:", sys.path)

try:
    import json
    print("Successfully imported json")
except Exception as e:
    print("Error importing json:", str(e))

try:
    from p2p_rust import (
        get_peer_id,
        get_storage_fragments,
        get_peer_with_most_space,
        get_token,
        get_contract_metadata
    )
    print("Successfully imported p2p_rust")
except Exception as e:
    print("Error importing p2p_rust:", str(e))

def process_packet(packet_json: str) -> Optional[str]:
    """
    Обработка входящего пакета с использованием функций Rust.
    """
    print("Processing packet...")
    try:
        # Преобразуем JSON строку в словарь
        packet = json.loads(packet_json)
        print("Packet data:", packet)

        # Пример использования функций из Rust
        if packet.get('act') == 'message' and 'data' in packet:
            if isinstance(packet['data'], dict) and 'text' in packet['data']:
                try:
                    # Получаем peer_id
                    peer_id = get_peer_id()
                    print("Got peer_id:", peer_id)
                    
                    # Получаем информацию о хранилище
                    storage_info = json.loads(get_storage_fragments())
                    print("Got storage info:", storage_info)
                    
                    # Получаем peer с наибольшим пространством
                    peer_with_space = json.loads(get_peer_with_most_space())
                    print("Got peer with space:", peer_with_space)
                    
                    # Получаем токен для peer
                    token_info = json.loads(get_token(peer_id))
                    print("Got token info:", token_info)
                    
                    # Добавляем полученную информацию в пакет
                    packet['data']['peer_info'] = {
                        'peer_id': peer_id,
                        'storage': storage_info,
                        'peer_with_space': peer_with_space,
                        'token': token_info
                    }
                except Exception as e:
                    print("Error in process_packet:", str(e))
        
        # Возвращаем модифицированный пакет как JSON строку
        return json.dumps(packet)
    except Exception as e:
        print("Error processing packet:", str(e))
        return packet_json  # Возвращаем оригинальный пакет в случае ошибки

if __name__ == "__main__":
    # Тестовый код
    test_packet = {
        "act": "message",
        "data": {
            "text": "Test message",
            "contract_id": "test_contract_123"
        }
    }
    result = process_packet(json.dumps(test_packet))
    print(json.dumps(json.loads(result), indent=2)) 