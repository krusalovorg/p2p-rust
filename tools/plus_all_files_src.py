import os

def read_all_files(directory):
    all_content = []
    total_lines = 0
    
    for root, dirs, files in os.walk(directory):
        for file in files:
            file_path = os.path.join(root, file)
            if file.endswith('.rs'):
                try:
                    with open(file_path, 'r', encoding='utf-8') as f:
                        content = f.read()
                        lines = content.split('\n')
                        total_lines += len(lines)
                        all_content.append(f"\n\n=== {file_path} ===\n")
                        all_content.append(content)
                except Exception as e:
                    print(f"Ошибка при чтении файла {file_path}: {e}")
    
    return ''.join(all_content), total_lines

def main():
    src_dir = './src'
    output_file = 'code.txt'
    
    if not os.path.exists(src_dir):
        print(f"Директория {src_dir} не существует!")
        return
    
    content, total_lines = read_all_files(src_dir)
    
    with open(output_file, 'w', encoding='utf-8') as f:
        f.write(content)
    
    print(f"Все файлы успешно объединены в {output_file}")
    print(f"Общее количество строк кода: {total_lines}")

if __name__ == '__main__':
    main()
