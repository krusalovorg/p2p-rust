# client.py
import socket
import threading

def handle_remote(remote_socket, local_host, local_port):
    try:
        # Подключаемся к локальному хосту
        local_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        local_socket.connect((local_host, local_port))

        # Перенаправляем данные между удаленным хостом и локальным хостом
        while True:
            data = remote_socket.recv(4096)
            if len(data) == 0:
                break
            local_socket.sendall(data)
            response = local_socket.recv(4096)
            remote_socket.sendall(response)
    finally:
        remote_socket.close()
        local_socket.close()

def start_client(remote_host, remote_port, local_host, local_port):
    client = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    client.connect((remote_host, remote_port))
    print(f"[*] Connected to {remote_host}:{remote_port}")

    remote_handler = threading.Thread(
        target=handle_remote, args=(client, local_host, local_port)
    )
    remote_handler.start()

if __name__ == "__main__":
    REMOTE_HOST = "86.110.212.133"
    REMOTE_PORT = 3030
    LOCAL_HOST = "localhost"
    LOCAL_PORT = 3030

    start_client(REMOTE_HOST, REMOTE_PORT, LOCAL_HOST, LOCAL_PORT)