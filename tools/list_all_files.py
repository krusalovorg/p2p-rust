import os

def list_all_files(directory):
    all_files = []
    excluded_dirs = {'target', 'tests', '.git', '.vscode'}
    
    for root, dirs, files in os.walk(directory):
        dirs[:] = [d for d in dirs if 'storage' not in d.lower()]
        dirs[:] = [d for d in dirs if d not in excluded_dirs]
        
        for file in files:
            file_path = os.path.join(root, file)
            all_files.append(file_path)
    
    return all_files

def main():
    src_dir = '../'
    output_file = 'file_list.txt'
    
    if not os.path.exists(src_dir):
        print(f"Директория {src_dir} не существует!")
        return
    
    files = list_all_files(src_dir)
    
    with open(output_file, 'w', encoding='utf-8') as f:
        for file_path in files:
            f.write(f"{file_path}\n")
    
    print(f"Список всех файлов сохранен в {output_file}")
    print(f"Всего найдено файлов: {len(files)}")

if __name__ == '__main__':
    main() 