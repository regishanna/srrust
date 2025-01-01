import socket
import threading
import argparse


def create_connection(target_ip, target_port):
    try:
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.connect((target_ip, target_port))
        print(f"Connected to {target_ip}:{target_port}")

        # Send a position message : (lat = 46.344877, long = -1.466214)
        pos_msg = b"\x00\x08\x02\xc3\x2a\xad\xff\xe9\xa0\x9a"
        sock.sendall(pos_msg)

        # Receive aircraft positions until peer closes the connection
        while True:
            data = sock.recv(1024)
            if not data:
                break

        sock.close()
        print(f"Disconnected to {target_ip}:{target_port}")

    except Exception as e:
        print(f"Failed to connect to {target_ip}:{target_port} - {e}")


def main():
    parser = argparse.ArgumentParser(description="Client connection simulator.")
    parser.add_argument("--ip", type=str, required=True, help="Target address")
    parser.add_argument("--port", type=int, default=1664, help="Target port")
    parser.add_argument("--connections", type=int, default=10, help="Number of connections to create")
    args = parser.parse_args()

    threads = []
    for _ in range(args.connections):
        thread = threading.Thread(target=create_connection, args=(args.ip, args.port))
        threads.append(thread)
        thread.start()

    for thread in threads:
        thread.join()


if __name__ == "__main__":
    main()